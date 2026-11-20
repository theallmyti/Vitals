# Cookie & Storage Notice — Vitals

**Effective date:** 11 November 2026 · **Last updated:** 20 November 2026
**Version:** 2.1

This notice explains what Vitals stores on your browser, computer and phone. It is part of our [Privacy Policy](PRIVACY.md).

## 1. Web dashboard

The Vitals web dashboard **does not set any cookies itself** and contains **no advertising, analytics or tracking scripts**.

It stores one item in your browser's `localStorage`:

| Name | Purpose | Type | How long |
|---|---|---|---|
| `pcm-session` | Your sign-in token, so you stay signed in | Strictly necessary | Until you sign out or delete your account. The backend also stops accepting it after 30 days without use. |

The dashboard is hosted on **Cloudflare Pages**. Cloudflare may set strictly necessary security cookies (for example, to block bots or attacks) when its protection systems require it. These are not used for tracking or advertising.

Because the dashboard only uses storage that is strictly necessary to provide the service you asked for, it does not need a consent banner under Article 5(3) of the EU ePrivacy Directive (2002/58/EC) and equivalent laws.

**To clear it:** click **Sign out**, or clear site data for the dashboard in your browser settings.

## 1a. Website (vitals.dpdns.org)

The Vitals website **sets no cookies** and contains **no advertising or tracking scripts**. It uses Vercel Web Analytics to count page views, which works without cookies or any other stored identifier (see Privacy Policy Section 3.7).

If you turn on the optional ambient sound, it stores one item in your browser's `localStorage`:

| Key | Purpose | How long |
|---|---|---|
| `vitals-sound` | Remembers whether you switched the ambient sound on or off | Until you clear site data for vitals.dpdns.org |

It is created only when you press the sound button, is never sent anywhere, and the page works the same without it.

The website is hosted on **Vercel**, which may set strictly necessary security cookies if its protection systems require it.

## 2. Android app

The Android app stores on your phone (in app-private storage): your sign-in token, settings and alert thresholds.

The app also includes Google Firebase:

| Technology | Provider | Purpose | Type |
|---|---|---|---|
| Firebase Cloud Messaging token | Google | Deliver alert notifications | Necessary for notifications |
| **Google Analytics for Firebase**: app-instance ID and Android Advertising ID | Google | App-usage statistics | **Analytics (optional)** |

These work like cookies inside the app. See Privacy Policy Section 3.4 for exactly what Google Analytics for Firebase collects. You can reset or delete your Advertising ID in Android Settings → Privacy → Ads, and stop all collection by uninstalling the app.

## 3. Vitals Agent

The agent stores its configuration (including the device's secret key), status files and logs in its own folder on your computer:

- Windows: `%ProgramData%\Vitals`
- Linux: `/etc/vitals` (the config file is root-only, `chmod 600`)

These files stay on your computer and are not cookies. Uninstalling the agent removes them.

## 4. Changes

If we ever add non-essential cookies or trackers to the web dashboard, we will update this notice first and ask for your consent where the law requires it.

## 5. Contact

contact@vitals.dpdns.org
