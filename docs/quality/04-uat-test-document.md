# Talent Scout Desktop — test document for recruiters

Document version: 1.0  
Date: 19 October 2026  
App: Talent Scout Desktop, quality build after commits `735fc1f`, `8dc47a3`, and `c38a402`.  
Tracker: `docs/quality/04-uat-test-document.xlsx`

## 1. Purpose, scope, and what you need

Purpose: confirm a recruiter can screen resumes on their own laptop without training and without sending resumes anywhere.

In scope: install, read a folder, rank a job, filter, Excel, Start fresh, email drafts, the sample portal, mistakes, privacy, and speed.

Out of scope: a live job site such as Naukri, sending email, and reading a photo of a resume.

What you need:

- Windows 10 or 11, 64-bit.
- The setup file for this quality build (the file whose name ends in `setup.exe`). The copy published as v0.1.0 on GitHub is the earlier build. Use a setup made after the quality commits.
- No administrator password.
- No extra programs. The setup already includes the window piece.
- Sample resumes: open the app and click **Try the sample resumes**. That loads three people and a sample job.
- Email: click **Connect email** inside the app. This does not open Gmail or Outlook, and it does not send mail. It only allows drafts to be saved.
- Sample portal: no account. It is practice data stored in the app.

## 2. Test data

| Pack | How to load it |
| --- | --- |
| Sample pack | **Try the sample resumes**. Ananya Rao and Meera Iyer are in Bengaluru. Rahul Menon is in Pune. |
| Your own folder | Put PDF, Word, or text resumes in one folder, including a subfolder if you want. Click **Choose resume folder**. |
| Developer pack | 206 synthetic files covering phones, notice wording, bad files, and a zip. Built by the engine test `probe_round1_records_current_behaviour`. Not needed for this recruiter pass. |

Do not use a real candidate’s resume in a shared test log.

## 3. How to record a result

Use the Excel tracker. For each case fill Actual result, Pass or Fail, your name, the date, and remarks. Leave them blank until you run the case.

Severity if you find a problem:

| Severity | Meaning |
| --- | --- |
| Critical | Resumes leave the laptop, a score is wildly wrong, or the app will not open. |
| High | A recruiter cannot finish the four steps, or a field they rely on is wrong for many files. |
| Medium | One path is confusing or one file type is weak, and there is a way around it. |
| Low | Wording, spacing, or a rare file. |

## 4. Test cases

### Install, update, uninstall

| ID | Area | Scenario | Preconditions | Steps | Expected result | Actual result | Pass/Fail | Tester | Date | Remarks |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| T-01 | Install | Clean install | Windows 10 or 11, app not installed | Download the setup file. Open it. If Windows says it protected your PC, choose More info, then Run anyway. Finish setup. | No administrator password is asked. Talent Scout Desktop appears in the Start menu and opens. | | | | | |
| T-02 | Install | Upgrade | An older setup is already installed | Run the newer setup file. | The app opens and previous behaviour still works. Your current search is still there unless it is older than 7 days. | | | | | |
| T-03 | Install | Uninstall and keep data | App has been used | Settings, Apps, uninstall. When asked to delete saved resumes, choose No. | The app is gone. Saying No keeps the saved search. | | | | | |
| T-04 | Install | Uninstall and delete data | App has been used | Uninstall and choose Yes to delete. | The saved search for this app is removed. | | | | | |

### Folder reading

| ID | Area | Scenario | Preconditions | Steps | Expected result | Actual result | Pass/Fail | Tester | Date | Remarks |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| T-10 | Folder | Read the sample pack | App is open | Click Try the sample resumes. | Three candidates appear after you rank. A photo is not required. | | | | | |
| T-11 | Folder | Subfolders and a second pass | A folder with resumes in a subfolder | Choose the folder. When it finishes, choose the same folder again. | The second pass says the files are unchanged and finishes quickly. | | | | | |
| T-12 | Folder | Pause and cancel | A folder with many files | Start a folder. Press Pause, then Resume. On another run, press Cancel. | Pause stops the count, Resume finishes it. Cancel stops and tells you it was cancelled. Other files already read stay. | | | | | |
| T-13 | Folder | Bad files | Folder contains an empty file, a photo, and a locked PDF, plus good resumes | Choose the folder. | Good resumes are listed. Each bad file has one sentence saying what happened and what to do. The others are not blocked. | | | | | |

### Job and ranking

| ID | Area | Scenario | Preconditions | Steps | Expected result | Actual result | Pass/Fail | Tester | Date | Remarks |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| T-20 | Job | Rank the sample job | Sample resumes loaded | Paste a Java, Spring Boot, Kafka job for Bengaluru, 5 to 9 years. Click See ranked candidates. | Ananya Rao is above Rahul Menon. Hover the match score. One sentence explains skills, experience, and city. | | | | | |
| T-21 | Job | A different job on the same folder | T-20 is done | Replace the job with a Python, Django job for Pune. Rank again. | Rahul Menon moves above the Java candidates. | | | | | |
| T-22 | Job | JS means JavaScript | A resume says JS | Rank a job that asks for JavaScript. | JS is treated as JavaScript in Matching skills. | | | | | |
| T-23 | Job | Job with no skills | Folder already read | Paste “Hiring now” and rank. | Match score is 0. The sentence says skills were not listed. | | | | | |

### Filters

| ID | Area | Scenario | Preconditions | Steps | Expected result | Actual result | Pass/Fail | Tester | Date | Remarks |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| T-30 | Filters | Notice and city together | Ranked sample or a larger folder | Set Notice to Up to 30 days. Tick Bengaluru. Click Apply filters. | Only people in Bengaluru who can join within 30 days remain. Immediate (0 days) is included. Serving notice without a date is not included in Up to 30 days. | | | | | |
| T-31 | Filters | No matches | A ranked list | Type a name that is not in the list. Apply filters. | The table says no candidates yet, and nothing crashes. | | | | | |
| T-32 | Filters | Minimum score | A ranked list | Set Minimum match score to 80. Apply filters. | Only rows with a match score of 80 or more remain. | | | | | |

### Excel

| ID | Area | Scenario | Preconditions | Steps | Expected result | Actual result | Pass/Fail | Tester | Date | Remarks |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| T-40 | Excel | Export the rows on screen | T-30 filter is applied | Click Download Excel and save the file. Open it in Excel or LibreOffice. | Columns are Rank, Name, Email, Phone, Location, Skills, Matching skills, Missing skills, Company, Notice, Match score, Note. Notice shows days, such as 30 days. The row count matches the screen. The header row stays visible when you scroll. | | | | | |

### Start fresh

| ID | Area | Scenario | Preconditions | Steps | Expected result | Actual result | Pass/Fail | Tester | Date | Remarks |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| T-50 | Start fresh | New folder asks first | A folder has already been read | Choose a different folder. | The app asks: Start a new search? People from the previous folder will be cleared. No is available. Yes clears the previous people and reads the new folder. | | | | | |
| T-51 | Start fresh | Start fresh | Any search is open | Click Start fresh. | The list is empty and the message says the saved search was cleared. | | | | | |
| T-52 | Start fresh | Seven days | A search was saved more than 7 days ago (a developer can set this) | Close and open the app. | A message says the previous search was cleared because it was more than 7 days old. | | | | | |

### Email drafts

| ID | Area | Scenario | Preconditions | Steps | Expected result | Actual result | Pass/Fail | Tester | Date | Remarks |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| T-60 | Email | Connect and save three drafts | Ranked candidates with email addresses | Click Connect email. Tick three people. Write a subject and a message. Click Save drafts. | The app says the drafts were saved and nothing was sent. No mail leaves the laptop. | | | | | |
| T-61 | Email | Edit one draft | T-60 done | Change the message and save again for one person. | The new wording is what was saved. It is still not sent. | | | | | |
| T-62 | Email | Disconnect | Email is connected | Click Disconnect email. Try to save a draft. | The app tells you to connect again. Nothing is sent. | | | | | |

### Sample portal

| ID | Area | Scenario | Preconditions | Steps | Expected result | Actual result | Pass/Fail | Tester | Date | Remarks |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| T-70 | Sample portal | Find and save | A job is pasted | Click Find sample profiles for this job. Tick two people. Click Save selected resumes to a folder. | The note says this is practice data, not a live job site. Two resume files appear in the folder. A download log has counts only. Saving the same people again skips them. | | | | | |
| T-71 | Sample portal | Read what you saved | T-70 folder exists | Choose that folder as the resume folder and confirm the new search. Rank the same job. | The saved people appear as candidates. | | | | | |

### Mistakes

| ID | Area | Scenario | Preconditions | Steps | Expected result | Actual result | Pass/Fail | Tester | Date | Remarks |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| T-80 | Mistakes | Wrong or missing folder | App is open | Cancel the folder picker, or point at a folder that was deleted. | The app says the folder could not be opened and tells you to choose the resume folder. | | | | | |
| T-81 | Mistakes | Empty folder | An empty folder | Choose it. | The list stays empty. The app does not crash. | | | | | |
| T-82 | Mistakes | Wrong file types | A folder of photos and spreadsheets only | Choose it. | Photos say they were skipped. Other types say to use PDF, Word, or text. | | | | | |
| T-83 | Mistakes | No job | Resumes are loaded, job box empty | Click See ranked candidates. | The app says to paste the job first. | | | | | |
| T-84 | Mistakes | No internet | Wi-Fi off | Rank a folder and open the sample portal. | Ranking still works. The sample portal still lists practice profiles. Nothing tries to reach a website. | | | | | |
| T-85 | Mistakes | Expired email | A developer sets the email login as already expired, or you wait past the 30-day connection | Try to save a draft. | The app says the email login has expired, connect again, and nothing was sent. | | | | | |

### Privacy and speed

| ID | Area | Scenario | Preconditions | Steps | Expected result | Actual result | Pass/Fail | Tester | Date | Remarks |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| T-90 | Privacy | Where data lives | You have ranked a folder | Ask the developer to open the saved-search file, or follow the note in the app header. | The header says resumes stay on this computer. The saved file does not contain a readable email address. Start fresh removes it. | | | | | |
| T-91 | Speed | One thousand text resumes | A developer folder of 1,000 one-page text resumes | Choose the folder and note the time. | Finishes in under 2 minutes on a normal laptop. | | | | | |

## 5. Defect template and exit checklist

When a case fails, record:

- ID from this document
- Title
- Severity (table in section 3)
- What you clicked, in order
- What you expected
- What happened
- A screenshot or the file name
- The area (folder, job, Excel, email, sample portal)

Go / no-go:

- Go when T-01, T-10, T-20, T-21, T-30, T-40, T-50, T-51, T-60, T-70, T-80, and T-83 pass, and there is no open Critical or High issue.
- No-go when any of those fail, when a score cannot be explained, or when anything is sent off the laptop.
- Medium issues can ship with a written note. Low issues can ship.

## 6. One-page quick start

1. Install from the setup file. If Windows warns you, choose More info, then Run anyway.
2. Open Talent Scout Desktop.
3. Click **Choose resume folder**, or **Try the sample resumes**.
4. Paste the job. Click **See ranked candidates**.
5. Set notice to **Up to 30 days** and tick a city. Click **Apply filters**.
6. Click **Download Excel**, or tick people and **Save drafts**. Drafts are not sent.
7. Click **Start fresh** when you want this computer cleared.

The match score is 0 to 100. Higher means closer to the job. Hover the score to read the one-sentence reason. Blank means the resume did not say that fact. The app does not guess.

## FAQ from this review

**Why does Windows say it protected my PC?**  
The setup is not from a paid certificate yet. More info, then Run anyway, is expected.

**Did my resumes get uploaded?**  
No. Ranking and the sample portal stay on this computer.

**What is the sample portal?**  
Practice profiles built into the app so you can try a download. It is not Naukri or any live site.

**Why was a photo skipped?**  
This setup does not read pictures. Ask for a PDF or Word file with real text.

**Why is notice a number of days?**  
So filters and Excel mean the same thing. A month is counted as 30 days. Immediate is 0 days. Serving notice without a date stays “Serving notice”.

**What does Start fresh do?**  
It deletes the saved search and drafts on this computer. It does not delete the resume folder you picked.

**Why did it ask before opening another folder?**  
Only one search is kept. The previous people are cleared after you say yes.

## 7. Version history and how to update this document

| Version | Date | What changed |
| --- | --- | --- |
| 1.0 | 19 October 2026 | First recruiter test document for the quality build. |

When the app changes, add a row to the history, adjust the expected result for the cases that changed, and add a case for the new step. Do not rename an ID. Add a new ID instead. Update the Excel tracker to match this table, and leave Actual result and Pass/Fail blank for the next tester.
