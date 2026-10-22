<!--
SPDX-FileCopyrightText: 2026 Minux
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Google-Kalender einrichten

Atlas gleicht Termine mit Google ab, wenn du dich einmal im Browser
anmeldest. Google lässt Programme dafür nur mit einem eigenen
„OAuth-Client“ zu. Den legst du einmal kostenlos an – das dauert etwa zehn
Minuten. Danach merkt sich Atlas nur ein Zugriffs-Token, verschlüsselt auf
deinem Rechner (`systemd-creds`, sonst eine Datei nur für dich lesbar);
dein Google-Passwort sieht Atlas nie.

## 1. Projekt in der Google Cloud Console

1. Öffne <https://console.cloud.google.com/> und melde dich mit deinem
   Google-Konto an.
2. Oben links „Projekt auswählen“ → **Neues Projekt**, Name z. B.
   `Atlas`, **Erstellen**.

## 2. Google Calendar API einschalten

1. Im Projekt: **APIs & Dienste → Bibliothek**.
2. Nach **Google Calendar API** suchen und **Aktivieren**.
3. Ebenso die **CalDAV API** suchen und **Aktivieren** (Atlas gleicht die
   Termine über CalDAV ab).

## 3. Zustimmungsbildschirm (OAuth consent screen)

1. **APIs & Dienste → OAuth-Zustimmungsbildschirm** (bzw. „Google Auth
   Platform“).
2. Nutzertyp **Extern**, App-Name z. B. `Atlas`, deine E-Mail-Adresse als
   Support- und Entwickler-Kontakt.
3. Bereiche (Scopes): **…/auth/calendar**, dazu `openid` und `email`.
4. Unter **Testnutzer** deine eigene Gmail-Adresse eintragen. Die App
   bleibt im Status „Test“ – das genügt für dich, eine Prüfung durch Google
   ist nicht nötig.

> Hinweis: Im Status „Test“ laufen Anmeldungen nach sieben Tagen ab. Atlas
> meldet dann „Bei Google neu anmelden“ – ein Klick auf **Neu anmelden** in
> den Kalender-Einstellungen genügt. Wer das vermeiden will, stellt die App
> unter „Veröffentlichungsstatus“ auf „In Produktion“ (für den eigenen
> Gebrauch ohne Prüfung möglich; Google zeigt dann beim Anmelden einen
> Hinweis „nicht überprüft“, den du mit „Weiter“ bestätigst).

## 4. OAuth-Client anlegen

1. **APIs & Dienste → Anmeldedaten → Anmeldedaten erstellen →
   OAuth-Client-ID**.
2. Anwendungstyp **Desktop-App**, Name z. B. `Atlas`.
3. **Erstellen** – Google zeigt **Client-ID** (endet auf
   `.apps.googleusercontent.com`) und **Clientschlüssel** (beginnt mit
   `GOCSPX-`).

## 5. In Atlas anmelden

1. Atlas: **Einstellungen → Kalender** (oder ⚙ im Kalender) → **Google**.
2. **Client-ID** und **Client-Secret** (den Clientschlüssel) eintragen.
3. **Mit Google anmelden** – der Browser öffnet sich; Konto wählen und den
   Zugriff auf den Kalender erlauben. Danach zeigt die Seite „Atlas ist
   angemeldet“, das Fenster kannst du schließen.
4. Zurück in Atlas erscheinen deine Kalender; **Speichern** – fertig.

## Was du wissen solltest

- **Nur Termine:** Googles Kalender kennen keine Aufgaben. Aufgaben legt
  Atlas im Projektkalender an.
- **Ein Konto:** Atlas verbindet entweder Nextcloud oder Google. Beim
  Wechsel vergisst es die Anmeldung, die Kalenderauswahl und den Cache des
  anderen Kontos (auf dem Server wird nichts gelöscht).
- **Offline:** wie bei Nextcloud – Änderungen warten im Cache und gehen
  über, sobald Google erreichbar ist.
- **Zugriff entziehen:** <https://myaccount.google.com/permissions> → Atlas
  → Zugriff entfernen; in Atlas dann **Konto trennen**.
- Client-ID und Client-Secret stehen in `~/.config/atlas/config.toml`.
  Für Desktop-Apps gilt das Secret bei Google ausdrücklich nicht als geheim;
  das eigentliche Zugriffs-Token steht dort nie.
