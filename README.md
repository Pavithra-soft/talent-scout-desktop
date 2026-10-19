# Talent Scout Desktop

A recruiter installs one Windows setup file. Resumes are read from the computer. Nothing is uploaded.

## For the recruiter

The download link is the GitHub Release, not the source code.

1. Download the file whose name ends in `setup.exe`.
2. Open it. If Windows says it protected your PC, choose **More info**, then **Run anyway**.
3. Finish the setup. An administrator password is not required.
4. Open **Talent Scout Desktop** from the Start menu.
5. Choose the resume folder, paste the job, look at the ranked candidates, then download Excel or save an email draft.

The setup already includes the Windows piece the window needs (WebView2). There is nothing else to install.

PDF, Word (.docx and .doc), RTF, and text files are read. A photo or a scanned page with no text is skipped and listed, because this installer does not add a separate scanner program.

Uninstall from Windows **Settings → Apps**. It asks whether to delete the saved scores.

## What the app does

- Reads a folder, including subfolders, or a zip. A second pass skips files that have not changed.
- Fills name, email, Indian mobile numbers, location, skills, company, experience, and notice period. Unknown fields stay blank.
- Ranks against a job description with skill synonyms (`JS` = JavaScript, `k8s` = Kubernetes), experience, and location. Weights can be changed.
- Filters by notice period, location, name, skills, company, and score.
- Downloads an Excel file of the candidates on screen.
- Saves email drafts on this computer. The app does not send them.
- Can copy practice profiles from a sample portal into a folder you choose. A live job site is not searched.

Ranking runs on the computer. This version does not send resumes to a cloud model.

## Performance budget

On a normal laptop, 1,000 one-page text resumes should finish in under 2 minutes. The included engine test reads 1,000 text resumes and then skips them on a second pass; that run finished in under a second on an Apple Silicon developer machine. PDF files are slower than text files and should stay within about 10 minutes for 1,000 one-page PDFs.

## Build

The Windows installer is built by GitHub Actions when a version tag is pushed. A signed installer needs a Windows code-signing certificate, which this release does not include.
