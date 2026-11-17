// Transactional email (password reset codes) through Resend's REST API. Cloudflare Email Sending
// needs the Workers Paid plan; Resend's free tier (3,000 emails/month, 100/day) is plenty here.
// Environment variables (Convex dashboard → Settings → Environment Variables):
//   RESEND_API_KEY  Resend API key with "Sending access" (resend.com → API Keys)
//   EMAIL_FROM      sender on the domain verified in Resend, e.g. "Vitals <noreply@vitals.dpdns.org>"
// https://resend.com/docs/api-reference/emails/send-email
import { ConvexError } from "convex/values";

export const RESEND_SEND_URL = "https://api.resend.com/emails";
export const EMAIL_NOT_CONFIGURED = "Password reset email isn't set up on this server yet.";
const SEND_FAILED = "Couldn't send the email right now. Please try again in a minute.";

export function emailConfigured(): boolean {
  return Boolean(process.env.RESEND_API_KEY && process.env.EMAIL_FROM);
}

export async function sendEmail(to: string, subject: string, text: string, html: string): Promise<void> {
  const apiKey = process.env.RESEND_API_KEY;
  const from = process.env.EMAIL_FROM;
  if (!apiKey || !from) throw new ConvexError(EMAIL_NOT_CONFIGURED);

  let res: Response;
  try {
    res = await fetch(RESEND_SEND_URL, {
      method: "POST",
      headers: { Authorization: `Bearer ${apiKey}`, "Content-Type": "application/json" },
      body: JSON.stringify({ from, to, subject, text, html }),
    });
  } catch {
    throw new ConvexError(SEND_FAILED);
  }
  const body = (await res.json().catch(() => null)) as { id?: string; name?: string; message?: string } | null;
  if (!res.ok || !body?.id) {
    // Log the provider's reason for the owner (e.g. an unverified domain or a revoked key);
    // never echo it, or the key, to the caller.
    console.error("email send failed", res.status, body?.name ?? "", body?.message ?? "");
    throw new ConvexError(SEND_FAILED);
  }
}
