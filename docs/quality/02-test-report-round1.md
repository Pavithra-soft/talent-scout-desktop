# Test report — round 1

Date: 19 October 2026. Product: Talent Scout Desktop v0.1.0, engine as built before the fix phase. Synthetic data only. No real person’s resume was used.

## Environment

| Item | Value |
| --- | --- |
| Machine | macOS (Apple Silicon), local developer build |
| How the engine was run | `cargo test probe_round1_records_current_behaviour` |
| Harness | `src-tauri/src/round1_probe.rs` |
| Evidence file | `docs/quality/round1-evidence.txt` |
| Window installer | Not installed here. Windows setup is produced on a Windows build machine. This round inspected `tauri.conf.json`, the uninstall hook, and the release workflow. |
| Excel application | Excel and LibreOffice are not installed on this machine. The `.xlsx` file was opened as a zip and the header row was read from `xl/sharedStrings.xml`. |
| Live portal and mailbox | Not used. There is no connector in this build to point at a mock either. |

Dataset: 206 labeled files. Text, Word (`.docx` and `.doc`), RTF, PDF, PNG scans, a pipe-style table, empty files, corrupt PDFs, a locked-looking PDF, a zip of 12 resumes plus a `../` path, nested folders, duplicate-style repeats, Indian phone shapes, notice phrases including “LWD 15 Nov”, a non-English name, and one 16 MB text file.

## Results

| Area | Result | Notes |
| --- | --- | --- |
| Ingestion of 206 files | Pass, with one safety miss | 4.2 seconds, about 2,960 files/minute, including the 16 MB file. All 206 rows were stored. |
| Bad-file isolation | Pass | Empty files and photos failed on their own. The other files were still stored. |
| Re-scan cache | Pass for normal files, fail for zip | Second pass 10 ms. 175 unchanged files skipped. 19 previous failures retried. 12 zip entries were read again. |
| Pause and cancel | Pass | Cancel stopped at 24 of 80 files, state `cancelled`. Pause then resume finished 40 of 40. |
| Zip path escape | Pass | `../escape.txt` was not saved. |
| Name accuracy | Fail | Precision 1.000, recall 0.859 (146 true, 24 missed). |
| Email accuracy | Fail | Precision 0.924, recall 0.924 (159 true, 13 wrong). |
| Phone accuracy | Pass | Precision 1.000, recall 1.000 on labeled phones, including +91, spaces, hyphens, and a leading 0. |
| Location accuracy | Fail | Precision 1.000, recall 0.920 (14 missed). |
| Skills accuracy | Pass with a small miss | Precision 1.000, recall 0.971 (15 skill mentions missed). |
| Company accuracy | Fail | Precision 0.862, recall 0.857. |
| Notice accuracy | Fail | Precision 1.000, recall 0.838 (26 day-counts missed). “LWD 15 Nov” came back blank. Expected 27 days on this date. |
| Ranking order and synonyms | Pass | Java plus JS in Bengaluru scored 90. Python in Pune scored 5. The same inputs scored 90 again. |
| Short job with no skills | Fail | “Hiring now” scored 90 for an empty resume. “Need someone in Bengaluru soon” scored 80 for a Python resume in Pune. |
| Filters | Pass | Up to 30 days: 83 rows. Bengaluru: 34. Both, with a minimum score: 16. A name that does not exist: 0 rows. Sort is high score first. |
| Excel file shape | Pass for structure, fail for words | File opens as a spreadsheet package. Headers are Rank, Name, Email, Phone, Location, Skill Set, Matching Skills, Non-Matching Skills, Previous Company, Notice Period, Score. Notice cells keep the original sentence, not days. Filtered export wrote 83 rows, which matches the 30-day list. |
| Saved search lock | Fail | After a checkpoint, the database file contains the bytes `@example.com`. |
| Start fresh, replace prompt, 7-day expiry | Fail | No such actions exist. |
| Email drafts | Fail | No draft action exists. Nothing was sent because nothing can be sent. |
| Sample portal and bulk download | Fail | No portal action exists. |
| Audit log | Fail | No audit table. |
| Startup if the database is unreadable | Fail | Code ends the program with `expect("database")`. Not executed against a corrupt file in this run. |
| Installer on Windows 10 and 11 | Not run | Setup is per user, bundles the window component, and the uninstaller asks before deleting data (default No). No self-update. The published setup is unsigned. |
| Low disk and low memory | Not run | No Windows session was available. The 16 MB file was accepted into memory, which is the related risk. |

Phone formats in the set (`9876543210`, `+91 98765 43210`, `98765-43210`, `09876543210`) all became `+919876543210`. A too-short number stays blank. That part is solid.

Photos return “This is a picture without text, so it was skipped.” Empty files return “No text found in this file.” Those two sentences are already plain. The locked PDF is not.

## Performance

| Run | Time | Rate |
| --- | --- | --- |
| 206 mixed files, one of them 16 MB | 4,175 ms | 2,960 files/minute |
| Same folder again | 10 ms | 175 files skipped |
| 1,000 small text resumes (existing engine test) | under 1 second | well inside the 2-minute budget |

The budget for 1,000 one-page text resumes (under 2 minutes) holds. The 16 MB file is still read in full, so a much larger file would use the laptop’s memory. PDF timing at 1,000 files was not measured in this round.

## Defect log

### D-C1 — Candidate details are readable in the saved-search file

- Severity: Critical
- Steps: Read a folder of synthetic resumes. Checkpoint the database. Search the file for `@example.com`.
- Expected: The email is not present as plain text.
- Actual: `plaintext_email_in_db=true`.
- Evidence: `docs/quality/round1-evidence.txt`
- Component: saved search (`src-tauri/src/db.rs`)

### D-C2 — An unreadable saved search stops the program

- Severity: Critical
- Steps: Follow `run()` in `src-tauri/src/lib.rs`. The database open uses `expect("database")`.
- Expected: A sentence that tells the recruiter the saved search could not be opened, and a way to start again.
- Actual: The program is written to quit with a panic.
- Evidence: `src-tauri/src/lib.rs` setup block. Not triggered on a corrupt file in this run.
- Component: startup

### D-H1 — Previous candidates are kept with no Start fresh and no expiry

- Severity: High
- Steps: Search the commands for a clear, a confirm-before-replace, or a date check.
- Expected: A new folder asks before it replaces the current search. Start fresh deletes the saved search. Data older than 7 days is cleared.
- Actual: A new folder only changes which rows are shown. Old rows remain in the file.
- Evidence: commands in `src-tauri/src/lib.rs` are ingest, rank, list, and Excel only.
- Component: saved search

### D-H2 — A job with no skills still awards a high skill score

- Severity: High
- Steps: Score an empty resume against “Hiring now”. Score a Python resume in Pune against “Need someone in Bengaluru soon”.
- Expected: Skill points are not awarded when the job lists no skills.
- Actual: Scores were 90 and 80.
- Evidence: `empty_job_score=90`, `short_job_score=80` in the evidence file.
- Component: ranking (`src-tauri/src/rank.rs`)

### D-H3 — “LWD 15 Nov” is not turned into days, and the list shows the raw phrase

- Severity: High
- Steps: Read “Notice period: LWD 15 Nov”. Export Excel.
- Expected: 27 days on 19 October 2026, and the same day text in the list and in Excel.
- Actual: `lwd_days=None`. Excel stores “Notice: 15 days” and “Notice period: 30 days Skills: …” rather than a day count. Notice recall 0.838, 26 misses.
- Evidence: evidence file, Excel shared strings.
- Component: rules and Excel

### D-H4 — A 16 MB file is read and stored

- Severity: High
- Steps: Put a 16 MB text file in the folder and read the folder.
- Expected: The file is skipped with a sentence that it is too large, and it does not occupy a full read.
- Actual: `huge_error` is empty and `huge.person@example.com` was stored. This was the one error-flag mismatch (`error_mismatch=1`).
- Evidence: evidence file, file `bad/huge.txt` in the synthetic set.
- Component: folder reading

### D-H5 — A failed folder read can be thrown away

- Severity: High
- Steps: Read `start_ingest` in `src-tauri/src/lib.rs`.
- Expected: A failure becomes one sentence on the progress line.
- Actual: The result of `ingest_path` is ignored (`let _ = ingest::ingest_path`).
- Evidence: source inspection. This run’s folder succeeded, so the path was not shown on screen.
- Component: folder reading

### D-H6 — There is no email draft and no sample portal

- Severity: High
- Steps: Look for a draft action and a portal action in the window and the commands.
- Expected: A draft saved on this computer that is never sent, and a labelled sample portal that can copy chosen profiles into a folder.
- Actual: Neither action exists.
- Evidence: `src/App.tsx` ends at Excel. Command list in `lib.rs`.
- Component: reply and portal

### D-H7 — A bad PDF shows an engineer’s error

- Severity: High
- Steps: Read `bad/locked.pdf`.
- Expected: “This PDF could not be read. Ask for an unlocked copy.”
- Actual: `PDF error: Invalid cross-reference table (invalid start value)`.
- Evidence: `locked=` line in the evidence file.
- Component: PDF reader

### D-H8 — The window is not the four-step journey, and names do not match

- Severity: High
- Steps: Read the window labels and the Excel header row.
- Expected: Pick folder, add the job, see ranked candidates, export or reply. Headers use Skills, Missing skills, Company, Notice, Match score. Weights sit under Advanced. The job box starts empty.
- Actual: Three sections, “Rank profiles”, “Must-have weight” on the main screen, Excel headers “Skill Set”, “Non-Matching Skills”, “Previous Company”, “Notice Period”. The sample job is loaded at startup.
- Evidence: `src/App.tsx`, Excel shared strings.
- Component: window and Excel

### D-H9 — RTF resumes glue the next line onto the email and the company

- Severity: High
- Steps: Read `rtf/resume-220.rtf` through `resume-227.rtf`.
- Expected: `candidate220@example.com` and company `Infosys`.
- Actual: Email `candidate220@example.comphone`. Company `Infosys7 years experienceNotice period`. Several RTF names are blank. This drives most of the email, company, and name misses.
- Evidence: `misses:` lines in the evidence file.
- Component: RTF reader

### D-H10 — A one-line PDF pulls later words into the company

- Severity: High
- Steps: Open the Excel shared strings for a PDF row.
- Expected: Company is `Infosys` only.
- Actual: `Infosys 6 years experience Notice period`.
- Evidence: Excel shared strings in the evidence file.
- Component: company rule

### D-M1 — The same email is not marked

- Severity: Medium
- Steps: Two files can carry the same address (the set repeats addresses by design of the phone list, and a second email line is added on some files). There is no duplicate flag in the row.
- Expected: A later row says it shares an email with another candidate.
- Actual: No such note exists.
- Evidence: `Stored` has no note field (`src-tauri/src/db.rs`).
- Component: saved search

### D-M2 — “José Alvarez” is left blank

- Severity: Medium
- Steps: Read a text resume whose first line is `José Alvarez`.
- Expected: The name is kept.
- Actual: `jose_name=''`. The name filter for José returned 0 rows.
- Evidence: evidence file, `resume-7.txt`.
- Component: name rule

### D-M3 — A table line does not yield the name

- Severity: Medium
- Steps: Read `tables/columns.txt` (`Name | Priya Sharma`).
- Expected: Name Priya Sharma.
- Actual: Name blank. Email was found.
- Evidence: `table_name=""` in the evidence file.
- Component: name rule

### D-M4 — Zip entries are read again on every pass

- Severity: Medium
- Steps: Read a folder that contains `pack.zip`, then read it again without changes.
- Expected: Unchanged zip entries are skipped.
- Actual: Second pass parsed 12 files, which are the 12 zip entries. 175 other files were skipped.
- Evidence: `rescan_skipped=175 parsed=12`.
- Component: folder reading

### D-M5 — There is no audit line

- Severity: Medium
- Steps: Look for a log of folder read, rank, Excel, and drafts that omits names and emails.
- Expected: Counts and actions only.
- Actual: No audit table.
- Evidence: `db.rs` schema.
- Component: saved search

### D-M6 — The sample job is filled in before the recruiter types

- Severity: Medium
- Steps: Follow the window startup. `sample_jd` runs immediately.
- Expected: An empty job box and the hint “Paste the job here.”
- Actual: The sample job is loaded on open.
- Evidence: `src/App.tsx` effect.
- Component: window

### D-M7 — The setup is unsigned and does not update itself

- Severity: Medium
- Steps: Read the bundle settings and the release workflow.
- Expected: A signed setup that can update, or a written reason to wait.
- Actual: No certificate and no updater. Uninstall does ask before deleting data. Per-user install does not ask for an administrator password.
- Evidence: `src-tauri/tauri.conf.json`, `.github/workflows/release.yml`, `src-tauri/windows/hooks.nsh`.
- Component: installer

### D-L1 — The progress bar is hidden from assistive tech

- Severity: Low
- Steps: Read the progress markup.
- Expected: A name and a value for the bar.
- Actual: `aria-hidden="true"`.
- Evidence: `src/App.tsx`.
- Component: window

## What passed and is not a defect

- Indian mobile numbers in the four shapes above.
- Pause, cancel, and per-file isolation for empty files and photos.
- Zip paths that contain `..` are dropped.
- Match order, synonym `JS`, and a stable score when the same resume is scored twice.
- Filters combine, an unknown name returns no rows, and the list sorts by score.
- The Excel package has a header row, a freeze, and an autofilter, and a filtered export contains only the filtered count.
- Second pass skips unchanged normal files in 10 ms.
- Photos are skipped in a sentence a recruiter can read.

## Exit gate

Every area in the brief was either run on synthetic data or, where this Mac cannot run it (Windows install, Excel the application, LibreOffice, low disk, a live mailbox), inspected and written down. Every failure above is a defect with an id.
