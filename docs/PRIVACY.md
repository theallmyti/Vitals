# Privacy Policy — Vitals

**Effective date:** 11 November 2026 · **Last updated:** 20 November 2026
**Version:** 2.1

> **Plain-language summary.** Vitals shows your computers' hardware health (CPU, RAM, GPU, disks, battery, temperatures) on your own dashboard and phone. To do that, we store your email, a hash of your password, details about the computers you pair, and recent metrics. By default the agent also sends the names of the top 5 programs by memory use and when your computer booted, slept and woke; you can turn that off. The Android app uses Google Firebase for notifications and Google Analytics for Firebase for app-usage statistics. We do not sell your data. Minute-by-minute history is deleted after 7 days. You can export your data or delete your account at any time.

---

## 1. Who we are

Vitals is a free, open-source student project built and run by three individuals in Vadodara, Gujarat, India:

- **Aditya Prasad** — desktop agent and backend
- **Tirth Patel** — Android app and notifications
- **Pratham Vasava** — Android app user interface

Vitals is **not a company**. For the hosted Vitals service described below, the three of us jointly decide why and how your personal data is processed. That makes us the **Data Fiduciary** under India's Digital Personal Data Protection Act, 2023 ("DPDP Act") and the **controller** under the EU/UK General Data Protection Regulation ("GDPR"), where those laws apply.

**Privacy and grievance contact:** Aditya Prasad — contact@vitals.dpdns.org
Source code and public issues: https://github.com/theallmyti/Vitals
Please do not post personal information in public GitHub issues; use the email above.

## 2. What this policy covers

This policy covers the **hosted Vitals service** run by us:

- the **Vitals website** at vitals.dpdns.org,
- the **Vitals web dashboard**,
- the **Vitals Agent** (Windows and Linux) when it reports to our backend, and
- the **Vitals Android app**.

**Self-hosting:** Vitals is open-source. If you or anyone else runs Vitals with their **own** backend, we never receive that data, and whoever runs that backend is responsible for it. This policy does not cover modified versions or third-party deployments.

## 3. What we collect

### 3.1 Account data (you give us this)
| Data | Why |
|---|---|
| Email address | Your login and the address we contact you on |
| Password hash (scrypt with a unique random salt) | To check your password. We never store your plain password. |
| Account creation time, first-run setup status | To run the service |

### 3.2 Security and sign-in data (created when you use the service)
| Data | Why |
|---|---|
| Session record: a SHA-256 hash of your sign-in token and when it was last used | To keep you signed in. The token itself stays in your browser or phone. |
| Failed sign-in attempts (email + time) | To block password guessing |
| Rate-limit events (e.g. sign-ups, pairing attempts, with time) | To stop abuse |
| Pairing codes (code, device name, OS, status, expiry) | To link a computer to your account |

Our hosting providers also see your **IP address** and basic request data (see Section 6).

### 3.3 Device data (sent by the Vitals Agent on computers you pair)
| Category | Exactly what is sent |
|---|---|
| Device identity | Device name you chose, hostname, operating system and version, CPU model, RAM speed (MT/s), list of fixed drives and their sizes, whether it has a battery / NVIDIA GPU / temperature sensors, whether it is a Raspberry Pi, agent version, a SHA-256 hash of the device's secret key, pairing time, last-seen time |
| Live metrics (only while you are viewing the device) | CPU use (total and per core), clock speeds, CPU temperature, RAM and swap use, GPU temperature/use/VRAM/power/clock/fan (NVIDIA only), drive free space, battery level/charging/health/cycles/charge rate, uptime, and on Raspberry Pi the count of running/stopped/errored Docker containers |
| Minute summaries | Per minute: average and peak CPU and RAM use, CPU and GPU temperatures, GPU use, drive free space, battery level, and whether each sensor worked |
| **Activity data (on by default, can be turned off)** | The names of the **top 5 programs by memory use** (e.g. `chrome.exe`) with their memory in MB; the time the computer **booted**; recent **sleep and wake** times; and **total active time today** |

**About activity data.** Program names and boot/sleep/wake times can reveal what someone runs and when they use a computer. They are shown only to you on your dashboard and are not used for anything else. To stop sending them, add this line to the agent's `config.toml` (Windows: `%ProgramData%\Vitals\config.toml`; Linux: `/etc/vitals/config.toml`) and restart the agent:

```toml
share_activity = false
```

When turned off, the agent stops sending program names and boot/sleep/wake times, and the next update removes the previous values from your dashboard.

### 3.4 Android app data
| Data | Who receives it | Why |
|---|---|---|
| Your sign-in token, settings and alert thresholds | Stored on your phone; settings also sent to our backend | To run the app |
| **Firebase Cloud Messaging (FCM) token** | Google, and our backend once server-sent alerts are switched on | To send you alert notifications |
| **Google Analytics for Firebase**: app-instance ID, Android Advertising ID, device model, OS version, coarse location derived from IP address, app opens, screens viewed, session length, first launch | Google | To understand how the app is used and fix problems |
| **Camera** (only when you choose to scan a pairing QR code) | Processed on your phone by Google ML Kit. Camera images are not sent to us. | To read the pairing code |

You can reset or delete your Advertising ID in Android Settings → Privacy → Ads. Google's handling of Firebase data is described at https://firebase.google.com/support/privacy.

### 3.5 What we do NOT collect
Vitals does **not** collect: keystrokes or clipboard contents; screenshots, screen recordings, camera images (other than the on-phone QR scan above), or microphone audio; the contents of your files, documents or folders; browser history, URLs or search queries; window titles or program command lines; precise (GPS) location; MAC addresses; payment card details.

### 3.6 Data kept only on your computer
The agent keeps its configuration (including its secret key) and log files on your computer. They are not sent to us.

### 3.7 Visiting the Vitals website
The website at vitals.dpdns.org has no account, forms, cookies, analytics or advertising. When you visit it:

- **Vercel** serves the page and, like any web host, receives your IP address, browser details and the pages you request, which it keeps in short-lived request logs.
- **jsDelivr** serves the open-source 3D graphics library the page uses, and receives the same kind of request information.
- **GitHub** receives a request from your browser when you open the download window (to look up the newest release) and when you download a file.
- If you turn on the optional ambient sound, your browser remembers that choice in its own `localStorage`. It is never sent to us.

## 4. Why we use your data, and on what basis

| Purpose | India (DPDP Act) | EU/UK (GDPR) |
|---|---|---|
| Create and run your account, show your devices, send alerts | Your consent (s. 6), given when you sign up and pair a device | Contract (Art. 6(1)(b)) |
| Security: stop password guessing, abuse and overload | Your consent and the reasonable security safeguards the law requires (s. 8(5)) | Legitimate interests (Art. 6(1)(f)) |
| Activity data (program names, boot/sleep/wake) | Your consent; you can turn it off at any time | Contract (Art. 6(1)(b)); you can turn it off |
| App-usage statistics (Google Analytics for Firebase) | Your consent | Consent (Art. 6(1)(a)) |
| Respond to your requests and legal obligations | Legitimate use (s. 7) | Legal obligation (Art. 6(1)(c)) |

We do **not** sell, rent or trade your personal data. We do **not** use it for advertising or to build profiles of you. We do not make automated decisions that have legal or similarly significant effects on you.

**Withdrawing consent:** you can withdraw at any time by turning off activity data, uninstalling the agent or app, or deleting your account. Withdrawal does not affect processing that already happened. Without account and device data, the service cannot work.

## 5. Age requirement
Vitals is only for people **aged 18 or older**. You must confirm you are 18+ when you sign up. Under the DPDP Act, processing the data of anyone under 18 requires verifiable parental consent, which we do not collect, so we do not allow under-18 accounts. If we learn that an account belongs to someone under 18, we will delete it. If you believe a child has an account, email contact@vitals.dpdns.org.

## 6. Who processes your data for us

| Provider | What they do | Data they handle | Location |
|---|---|---|---|
| **Convex, Inc.** | Database, backend functions, real-time sync | All account, security and device data in Section 3.1–3.3 | United States (hosted on Amazon Web Services) |
| **Cloudflare, Inc.** | Hosts the web dashboard (Cloudflare Pages), CDN, DDoS protection, DNS for vitals.dpdns.org, and forwards mail sent to contact@vitals.dpdns.org to our inbox (Email Routing) | IP address, browser details and request logs when you load the dashboard; sender, recipient and content of emails you send to our contact address | Global network |
| **Vercel Inc.** | Hosts the Vitals website (vitals.dpdns.org) | IP address, browser details and request logs when you visit the website | United States and global network |
| **jsDelivr** (open-source CDN) | Delivers the 3D graphics library used by the website | IP address and browser details when the website loads | Global network |
| **GitHub, Inc.** | Hosts the source code and the release downloads; the website asks its API for the newest release | IP address and browser details when you open the download window or download a file | United States |
| **Google LLC (Firebase)** | Push notifications (FCM) and Google Analytics for Firebase in the Android app | FCM token and the analytics data in Section 3.4 | United States and global |
| **Resend, Inc.** | Sending password-reset emails | Your email address and the email content (the one-time reset code) | United States |

Each provider handles data under its own terms and security programme. Convex states that it is SOC 2 Type 2 compliant and encrypts stored data with 256-bit AES (https://www.convex.dev/security). We rely on these providers' published security measures; we do not run or audit their systems ourselves.

We may also disclose data if required by law, a court order or a government authority, or to protect the safety of users or the service.

## 7. Transfers outside India
Our providers store and process data in the United States and other countries. India currently allows such transfers except to countries the Central Government restricts (DPDP Act s. 16); the United States is not restricted at the time of writing. For EU/UK users, data is transferred to the United States under the safeguards each provider offers (such as Standard Contractual Clauses or the EU–US Data Privacy Framework, where the provider participates).

## 8. How long we keep data

| Data | Kept for |
|---|---|
| Live metrics | Replaced on every update; only the latest values are kept |
| Minute summaries | **7 days**, then deleted automatically every day |
| Sessions | Until you sign out, or 30 days without use |
| Failed sign-ins and rate-limit events | About 24 hours |
| Pairing codes | Deleted about 1 hour after they expire |
| Unpaired devices | Deleted after 30 days without activity |
| Account and paired devices | Until you delete your account or the device |
| Database backups | Up to the **8 most recent** backups, stored privately by the maintainer (Aditya Prasad). Older backups are deleted as new ones are made. Data you delete can remain in a backup until it is rotated out. |
| Google Analytics for Firebase | According to the retention setting in our Firebase project (set to the shortest option Google offers) and Google's policies |

## 9. Your rights

You can, at any time:

1. **Access and export** — click **Export My Data (JSON)** in the dashboard. The export contains your account email and creation date, your devices, each device's latest snapshot and its **most recent 100 minute summaries**. For a complete copy of everything we hold, email contact@vitals.dpdns.org.
2. **Correct** — email us to correct your account details. Device names and settings can be changed in the app.
3. **Delete** — click **Delete Account** in the dashboard. This immediately deletes your account, sessions, devices, live data, summaries and pairing records. Remaining traces: failed sign-in and rate-limit records (deleted within about 24 hours), backups (until rotated, Section 8), and Google Analytics data (held by Google; uninstalling the app stops further collection). You can also request deletion by email without using the app.
4. **Withdraw consent** — see Section 4.
5. **Nominate someone** (India) — name a person who can exercise your rights if you die or become unable to (DPDP Act s. 14). Email us to do this.
6. **Object or restrict** (EU/UK) — email us. You can also pause the agent from its tray menu, turn off activity data, or uninstall it.
7. **Complain** — email us first; we will respond within **30 days**. If you are not satisfied, you may complain to the **Data Protection Board of India** once it accepts complaints, or, in the EU/UK, to your local data protection authority.

We may need to confirm you own the account (for example, by asking you to email us from your account address) before acting on a request.

## 10. How we protect data

- Traffic between the agent, app, dashboard and backend uses HTTPS/TLS provided by Convex, Cloudflare and Google. The website is served over HTTPS by Vercel.
- Passwords are stored only as salted scrypt hashes. Sign-in tokens and device secrets are stored only as SHA-256 hashes.
- Every dashboard request checks that the device belongs to the signed-in user.
- Sign-ins, sign-ups and pairing attempts are rate-limited.
- On Windows, the installer restricts the agent's data folder to SYSTEM and Administrators. On Linux, the config file `/etc/vitals/config.toml` is root-only (`chmod 600`).
- Backups are kept off GitHub and stored privately.

**No system is completely secure.** We are three students running a free project, not a professional security team. Do not use Vitals on computers where exposing the data in Section 3 would put you at serious risk.

## 11. If there is a data breach
If we learn of a breach affecting your personal data, we will tell affected users by email without undue delay, explain what happened and what to do, and notify authorities where the law requires (including the Data Protection Board of India and CERT-In, where applicable).

## 12. Changes to this policy
We will post changes here and update the dates at the top. For material changes (for example, collecting a new type of data or adding a new provider), we will notify you by email or in the dashboard at least **7 days** before they take effect, unless the change is required sooner by law or for security. Previous versions are kept in the repository's Git history.

## 13. Contact
**Privacy and grievance contact:** Aditya Prasad — contact@vitals.dpdns.org
Address: Vadodara, Gujarat, India (full postal address available on request for legal notices)
