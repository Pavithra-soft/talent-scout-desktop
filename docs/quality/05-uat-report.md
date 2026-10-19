# UAT report

Date: 19 October 2026. Role: recruiter who has not read the code, checking the four steps on synthetic people only.

The six journeys were run through the same actions the window uses: read a folder, rank, filter, Excel, replace a folder, Start fresh, save drafts, and the sample portal. They live in `recruiter_scenarios_s1_to_s6`, which passed in under a second on this Mac. The window itself was built with the production build. It was not clicked on a Windows laptop in this pass, because this machine cannot install the Windows setup.

## Scenarios

| ID | What the recruiter did | Result |
| --- | --- | --- |
| S1 | Read 150 synthetic resumes for a Java role, keep Bengaluru and notice within 30 days, take the top 10, export Excel | Pass. Every kept person is in Bengaluru with notice of 30 days or less. The Excel file is written. |
| S2 | Rank the same people against a Python role in Pune | Pass. The Python candidate scores above the Java candidate. |
| S3 | Switch folder, confirm the previous people are cleared, then Start fresh | Pass. The second folder replaces the first only after the confirm check. Start fresh leaves no saved people. |
| S4 | Save drafts for three people, edit one, confirm nothing is sent | Pass. The edited file contains the new wording and `X-Unsent`. It does not say it was sent. |
| S5 | Sample portal for a Java job, save two profiles, read that folder | Pass. The note says practice data. Two files are saved. After a read, a name is present. The download log is not a resume. |
| S6 | Missing folder, empty folder, a photo, no job, expired email, no network | Pass. The missing folder says it could not be opened. An empty folder adds nobody. A photo is skipped in a sentence. An empty job scores 0. An expired login says to connect again and that nothing was sent. The sample portal still returns practice profiles with no network call. |

## What was confusing, and what changed

| Issue | Severity | What we changed | Re-checked |
| --- | --- | --- | --- |
| The first screen talked about weights, profiles, and files per minute | High | Four steps, Advanced closed, plain status line | Window source and build |
| The job box arrived already filled | Medium | It now starts empty, with “Paste the job here.” | Window source |
| Notice did not always show days | High | List and Excel use the same day text | Probe recall 1.000 and S1 |
| A new folder silently hid the old people but left them stored | High | The app asks, then clears | S3 |
| There was no way to reply or to try a portal without a live login | High | Drafts that are not sent, and a labelled sample portal | S4 and S5 |
| The progress bar was invisible to a screen reader | Low | It now has the name “Folder progress” and a value | Window source |
| The download log could have been read as another resume | Low | The log is `download-log.log`, which the reader ignores | Portal test |

No UX issue above Low remains open. The unsigned Windows warning is still real. It is explained in the quick start, and it is not a surprise behaviour inside the app.

## Trust

The header says resumes stay on this computer. The match score has a one-sentence reason on hover. Start fresh deletes the saved search. A search older than 7 days is cleared on the next open. The database file from the probe does not contain `@example.com` in plain text.

## Accessibility checked in the window source

- Buttons, fields, and the progress bar are normal controls, so Tab reaches them.
- Body text is dark on a light background. The font size is 16px.
- Focus shows a dark green outline.
- The progress bar is named.

A full keyboard pass on Windows is still for the recruiter using the tracker (cases T-01 and the main path).

## Remaining risks

- The setup file already on GitHub is the first build, before these fixes. Do not send that file for this test document.
- Windows SmartScreen will warn until a certificate is bought. Self-update stays off until then.
- Photos of resumes are skipped on purpose.
- Two files in the 206-file set with no name line still received a name. Rare, and low.
- This UAT did not click the installed Windows app. The engine journeys passed. A recruiter should still run T-01 on a Windows 10 or 11 PC before a wide send.
