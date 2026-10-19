# Quality summary

Date: 19 October 2026. Product: Talent Scout Desktop, the local app. The cloud site was not changed.

## What changed

The app is now a four-step search: pick a folder, add the job, see ranked candidates, export or reply. Candidate details in the saved search are encrypted. A new folder asks before it clears the previous people. Start fresh deletes that saved search, and a search older than 7 days is cleared on open. Notice is shown in days everywhere we measured. A job with no skills no longer gets a high score. Bad files get one plain sentence. Email drafts are saved on the laptop and are not sent. A sample portal can copy practice profiles into a folder. It does not log into a live job site.

## Test results

16 automated tests passed. On 206 synthetic resumes, email, phone, city, skills, company, and notice all came back with full recall. The folder read took 278 ms, and the second pass skipped unchanged files. The six recruiter journeys (screen and shortlist, a second job, switch folder and start fresh, three drafts, sample portal, and the deliberate mistakes) passed. The window build succeeded. A Windows install was not clicked on this Mac.

## Remaining risks

- The installer already published as v0.1.0 does not contain these fixes. Send a new setup built from this quality build.
- Windows will still warn that it protected the PC until a paid certificate exists. There is no self-update until then.
- Photos of resumes are skipped.
- A recruiter should run the install case on a Windows 10 or 11 PC before a wide rollout.

## Recommendation

**No-go** for sending the setup file that is on GitHub today.

**Go** for building a new Windows setup from this quality build and handing recruiters `docs/quality/04-uat-test-document.md` plus the Excel tracker. The engine and the six journeys are ready. The first Windows install on a real recruiter PC is the last check.
