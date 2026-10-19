import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open, save } from "@tauri-apps/plugin-dialog";

type Progress = {
  done: number;
  total: number;
  parsed: number;
  skipped: number;
  errors: number;
  files_per_min: number;
  current: string;
  state: string;
  messages: string[];
};

type Row = {
  rank: number;
  name: string;
  email: string;
  phone: string;
  location: string;
  skills: string;
  matching: string;
  missing: string;
  company: string;
  notice: string;
  score: number | null;
  rationale: string;
  note: string;
};

type Grid = {
  rows: Row[];
  locations: string[];
  errors: string[];
  root: string;
  offline: boolean;
};

type Filters = {
  notice: string;
  notice_min: number | null;
  notice_max: number | null;
  locations: string[];
  skills: string;
  company: string;
  name: string;
  score_min: number | null;
};

type PortalPerson = {
  id: string;
  name: string;
  email: string;
  phone: string;
  location: string;
  skills: string;
  company: string;
  years: number;
  notice: string;
};

const emptyFilters = (): Filters => ({
  notice: "any",
  notice_min: null,
  notice_max: null,
  locations: [],
  skills: "",
  company: "",
  name: "",
  score_min: null,
});

export default function App() {
  const [progress, setProgress] = useState<Progress | null>(null);
  const [grid, setGrid] = useState<Grid | null>(null);
  const [jd, setJd] = useState("");
  const [weights, setWeights] = useState({ must: 60, nice: 20, experience: 10, location: 10 });
  const [filters, setFilters] = useState<Filters>(emptyFilters());
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);
  const [pendingPath, setPendingPath] = useState("");
  const [selected, setSelected] = useState<string[]>([]);
  const [subject, setSubject] = useState("Your profile");
  const [draft, setDraft] = useState("We reviewed your resume and would like to talk.");
  const [mail, setMail] = useState("Email is not connected. Drafts stay on this computer and are not sent.");
  const [portalPeople, setPortalPeople] = useState<PortalPerson[]>([]);
  const [portalNote, setPortalNote] = useState("");
  const [portalPick, setPortalPick] = useState<string[]>([]);
  const filtersRef = useRef(filters);
  const jdRef = useRef(jd);
  const weightsRef = useRef(weights);
  const resultsRef = useRef<HTMLElement>(null);
  filtersRef.current = filters;
  jdRef.current = jd;
  weightsRef.current = weights;

  async function refresh(next?: Filters) {
    const result = await invoke<Grid>("list_results", { filters: next ?? filtersRef.current });
    setGrid(result);
    return result;
  }

  function showResults() {
    resultsRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  async function refreshMail() {
    const current = await invoke<{ connected: boolean; expired: boolean }>("mail_status");
    if (current.expired) {
      setMail("Your email login has expired. Connect again. Nothing was sent.");
    } else if (current.connected) {
      setMail("Email is connected for drafts only. Nothing is sent from this app.");
    } else {
      setMail("Email is not connected. Drafts stay on this computer and are not sent.");
    }
  }

  useEffect(() => {
    const unlisten = listen<Progress>("ingest-progress", (event) => {
      setProgress(event.payload);
      if (event.payload.state === "done" || event.payload.state === "cancelled") {
        setBusy(false);
        refresh()
          .then(async (loaded) => {
            if (event.payload.state === "cancelled") {
              setMessage("Reading stopped. Resumes already read are kept.");
              showResults();
              return;
            }
            const read = event.payload.parsed + event.payload.skipped;
            if (read === 0 && event.payload.errors === 0) {
              setMessage("This folder has no PDF, Word, or text resumes. Choose the folder that contains the files, then click Select Folder. The files do not have to appear in that window.");
              return;
            }
            const job = jdRef.current.trim();
            const ranked = job
              ? await invoke<Grid>("rank_profiles", { jdText: job, weights: weightsRef.current })
              : loaded;
            if (job) setGrid(ranked);
            const names = ranked.rows
              .map((row) => row.name)
              .filter((name) => name.trim().length > 0)
              .slice(0, 4);
            const who = names.length > 0 ? ` ${names.join(", ")}${ranked.rows.length > names.length ? `, and ${ranked.rows.length - names.length} more` : ""}.` : "";
            setMessage(
              job
                ? `Read ${ranked.rows.length} resumes and ranked them for this job.${who}`
                : `Read ${ranked.rows.length} resumes.${who} Paste the job, then click See ranked candidates.`,
            );
            showResults();
          })
          .catch((error: Error) => setMessage(plain(error)));
      }
    });
    invoke<string>("startup_note").then((note) => {
      if (note) setMessage(note);
    }).catch(() => undefined);
    refresh().catch(() => undefined);
    refreshMail().catch(() => undefined);
    const dropped = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "drop" && event.payload.paths[0]) {
        begin(event.payload.paths[0], false).catch((error: Error) => setMessage(plain(error)));
      }
    });
    return () => {
      unlisten.then((stop) => stop());
      dropped.then((stop) => stop());
    };
  }, []);

  async function begin(path: string, replace: boolean) {
    setMessage("");
    try {
      if (!replace) {
        const ready = await invoke<{ status: string }>("prepare_folder", { path });
        if (ready.status === "confirm") {
          setPendingPath(path);
          setMessage("Start a new search? People from the previous folder will be cleared from this computer.");
          return;
        }
      }
      setPendingPath("");
      setBusy(true);
      setSelected([]);
      await invoke("ingest_start", { path, replace });
    } catch (error) {
      setBusy(false);
      setMessage(plain(error));
    }
  }

  async function chooseFolder() {
    const selectedFolder = await open({ directory: true, multiple: false, title: "Choose the resume folder" });
    if (typeof selectedFolder === "string") {
      await begin(selectedFolder, false);
    }
  }

  async function chooseFiles() {
    const picked = await open({
      directory: false,
      multiple: true,
      title: "Choose the resume files",
      filters: [{ name: "Resumes", extensions: ["pdf", "docx", "doc", "rtf", "txt", "zip"] }],
    });
    const paths = Array.isArray(picked) ? picked : typeof picked === "string" ? [picked] : [];
    if (paths.length === 0) return;
    const folder = paths[0].replace(/[/\\][^/\\]+$/, "");
    await begin(folder, false);
  }

  async function useSamples() {
    setMessage("");
    try {
      const path = await invoke<string>("load_samples");
      const sample = await invoke<string>("sample_jd");
      setJd(sample);
      await begin(path, false);
    } catch (error) {
      setBusy(false);
      setMessage(plain(error));
    }
  }

  async function fresh() {
    setMessage("");
    try {
      await invoke("start_fresh");
      setJd("");
      setSelected([]);
      setPortalPeople([]);
      setPendingPath("");
      setFilters(emptyFilters());
      await refresh(emptyFilters());
      setMessage("Started fresh. The saved search on this computer has been cleared.");
    } catch (error) {
      setMessage(plain(error));
    }
  }

  async function rank() {
    setMessage("");
    try {
      const result = await invoke<Grid>("rank_profiles", { jdText: jd, weights });
      setGrid(result);
      setMessage(`${result.rows.length} candidates ranked on this computer.`);
    } catch (error) {
      setMessage(plain(error));
    }
  }

  async function download() {
    const dest = await save({
      defaultPath: "ranked-candidates.xlsx",
      filters: [{ name: "Excel", extensions: ["xlsx"] }],
    });
    if (typeof dest !== "string") return;
    try {
      const count = await invoke<number>("export_excel", { path: dest, filters });
      setMessage(`Saved ${count} candidates to Excel.`);
    } catch (error) {
      setMessage(plain(error));
    }
  }

  async function saveDrafts() {
    const people = (grid?.rows ?? [])
      .filter((row) => selected.includes(rowKey(row)))
      .map((row) => ({ name: row.name || "Candidate", email: row.email }));
    try {
      const count = await invoke<number>("save_drafts", { subject, body: draft, people });
      setMessage(`Saved ${count} email drafts on this computer. Nothing was sent.`);
      await refreshMail();
    } catch (error) {
      setMessage(plain(error));
    }
  }

  async function findPortal() {
    setMessage("");
    try {
      const result = await invoke<{ summary: string; people: PortalPerson[] }>("portal_search", { jdText: jd });
      setPortalPeople(result.people);
      setPortalNote(result.summary);
      setPortalPick([]);
    } catch (error) {
      setMessage(plain(error));
    }
  }

  async function savePortal() {
    const folder = await open({ directory: true, multiple: false, title: "Choose a folder for the sample resumes" });
    if (typeof folder !== "string") return;
    try {
      const result = await invoke<{ saved: number; skipped: number }>("portal_download", { path: folder, ids: portalPick });
      setMessage(`Saved ${result.saved} sample resumes. Skipped ${result.skipped} already in that folder.`);
    } catch (error) {
      setMessage(plain(error));
    }
  }

  function toggleCity(city: string) {
    setFilters((current) => {
      const locations = current.locations.includes(city)
        ? current.locations.filter((item) => item !== city)
        : [...current.locations, city];
      return { ...current, locations };
    });
  }

  function toggleRow(key: string) {
    setSelected((current) => current.includes(key) ? current.filter((item) => item !== key) : [...current, key]);
  }

  const percent = progress && progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : 0;
  const readCount = progress ? progress.parsed + progress.skipped : 0;
  const status = !progress
    ? "Choose the folder that holds the resumes, or choose the files you can see."
    : progress.state === "done" && readCount === 0 && progress.errors === 0
      ? "This folder has no PDF, Word, or text resumes."
      : progress.state === "done"
        ? `Read ${readCount} resume${readCount === 1 ? "" : "s"}${progress.errors > 0 ? `. ${progress.errors} could not be read` : ""}. They are listed below.`
        : progress.state === "running"
          ? `Reading resumes: ${progress.done} of ${progress.total}.`
          : `${labelState(progress.state)}: ${progress.done} of ${progress.total}.`;
  const seen = (grid?.rows ?? [])
    .map((row) => row.name.trim())
    .filter((name) => name.length > 0)
    .slice(0, 6);

  return (
    <main>
      <header>
        <h1>Talent Scout Desktop</h1>
        <p>Resumes stay on this computer. Nothing is uploaded.</p>
      </header>

      <section>
        <h2>1. Pick a resume folder</h2>
        <div className="row">
          <button type="button" onClick={chooseFolder} disabled={busy}>Choose resume folder</button>
          <button type="button" className="secondary" disabled={busy} onClick={chooseFiles}>Choose resume files</button>
          <button type="button" className="secondary" disabled={busy} onClick={useSamples}>Try the sample resumes</button>
          <button type="button" className="secondary" disabled={!busy} onClick={() => invoke("ingest_pause")}>Pause</button>
          <button type="button" className="secondary" disabled={!busy} onClick={() => invoke("ingest_resume")}>Resume</button>
          <button type="button" className="secondary" disabled={!busy} onClick={() => invoke("ingest_cancel")}>Cancel</button>
          <button type="button" className="secondary" onClick={fresh}>Start fresh</button>
        </div>
        <div className="progress" role="progressbar" aria-label="Folder progress" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent}>
          <span style={{ width: `${percent}%` }} />
        </div>
        <p className="muted">{status}</p>
        <p className="muted">In the folder window, the resume files may not be listed. That is normal. Click the folder, then Select Folder. Or click Choose resume files and pick the PDFs and Word files you can see. PDF, Word, and text files are read. Nothing is uploaded.</p>
        {seen.length > 0 && (
          <p>Resumes read: {seen.join(", ")}{(grid?.rows.length ?? 0) > seen.length ? `, and ${(grid?.rows.length ?? 0) - seen.length} more` : ""}.</p>
        )}
        {message && <p>{message}</p>}
        {progress && progress.messages.length > 0 && (
          <ul className="error">{progress.messages.map((item) => <li key={item}>{item}</li>)}</ul>
        )}
        {pendingPath && (
          <div className="row">
            <button type="button" onClick={() => begin(pendingPath, true)}>Yes, start a new search</button>
            <button type="button" className="secondary" onClick={() => { setPendingPath(""); setMessage(""); }}>Keep the current search</button>
          </div>
        )}
      </section>

      <section>
        <h2>2. Add the job</h2>
        <label>Job
          <textarea rows={8} value={jd} placeholder="Paste the job here." onChange={(event) => setJd(event.target.value)} />
        </label>
        <details>
          <summary>Advanced</summary>
          <div className="weights">
            <label>Required skills <input type="number" value={weights.must} onChange={(event) => setWeights({ ...weights, must: Number(event.target.value) })} /></label>
            <label>Extra skills <input type="number" value={weights.nice} onChange={(event) => setWeights({ ...weights, nice: Number(event.target.value) })} /></label>
            <label>Experience <input type="number" value={weights.experience} onChange={(event) => setWeights({ ...weights, experience: Number(event.target.value) })} /></label>
            <label>City <input type="number" value={weights.location} onChange={(event) => setWeights({ ...weights, location: Number(event.target.value) })} /></label>
          </div>
        </details>
        <button type="button" onClick={rank}>See ranked candidates</button>
        <p className="muted">JS counts as JavaScript. k8s counts as Kubernetes. Blank fields stay blank. Hover a match score to read why.</p>
      </section>

      <section ref={resultsRef}>
        <h2>3. See ranked candidates</h2>
        <div className="row">
          <label>Notice
            <select value={filters.notice} onChange={(event) => setFilters({ ...filters, notice: event.target.value })}>
              <option value="any">Any</option>
              <option value="immediate">Immediate or serving notice</option>
              <option value="15">Up to 15 days</option>
              <option value="30">Up to 30 days</option>
              <option value="60">Up to 60 days</option>
              <option value="90">Up to 90 days</option>
              <option value="custom">Custom range</option>
            </select>
          </label>
          {filters.notice === "custom" && (
            <>
              <label>From days <input type="number" value={filters.notice_min ?? ""} onChange={(event) => setFilters({ ...filters, notice_min: event.target.value === "" ? null : Number(event.target.value) })} /></label>
              <label>To days <input type="number" value={filters.notice_max ?? ""} onChange={(event) => setFilters({ ...filters, notice_max: event.target.value === "" ? null : Number(event.target.value) })} /></label>
            </>
          )}
          <label>Name <input type="text" value={filters.name} onChange={(event) => setFilters({ ...filters, name: event.target.value })} /></label>
          <label>Skills <input type="text" value={filters.skills} onChange={(event) => setFilters({ ...filters, skills: event.target.value })} /></label>
          <label>Company <input type="text" value={filters.company} onChange={(event) => setFilters({ ...filters, company: event.target.value })} /></label>
          <label>Minimum match score <input type="number" value={filters.score_min ?? ""} onChange={(event) => setFilters({ ...filters, score_min: event.target.value === "" ? null : Number(event.target.value) })} /></label>
        </div>
        <p>City</p>
        <div className="row">
          {(grid?.locations ?? []).map((city) => (
            <label key={city}>
              <input type="checkbox" checked={filters.locations.includes(city)} onChange={() => toggleCity(city)} /> {city}
            </label>
          ))}
          {(grid?.locations ?? []).length === 0 && <span className="muted">Cities appear after resumes are read.</span>}
        </div>
        <button type="button" className="secondary" onClick={() => refresh().catch((error: Error) => setMessage(plain(error)))}>Apply filters</button>
        {message && <p>{message}</p>}
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Pick</th><th>Rank</th><th>Name</th><th>Email</th><th>Phone</th><th>Location</th><th>Skills</th><th>Matching skills</th><th>Missing skills</th><th>Company</th><th>Notice</th><th>Match score</th>
              </tr>
            </thead>
            <tbody>
              {(grid?.rows ?? []).map((row) => {
                const key = rowKey(row);
                return (
                  <tr key={key}>
                    <td><input type="checkbox" aria-label={`Pick ${row.name || row.email || "candidate"}`} checked={selected.includes(key)} onChange={() => toggleRow(key)} /></td>
                    <td>{row.rank}</td>
                    <td>{row.name}{row.note ? <div className="muted">{row.note}</div> : null}</td>
                    <td>{row.email}</td>
                    <td>{row.phone}</td>
                    <td>{row.location}</td>
                    <td>{row.skills}</td>
                    <td>{row.matching}</td>
                    <td>{row.missing}</td>
                    <td>{row.company}</td>
                    <td>{row.notice}</td>
                    <td title={row.rationale}>{row.score ?? ""}</td>
                  </tr>
                );
              })}
              {(grid?.rows ?? []).length === 0 && (
                <tr><td colSpan={12}>No candidates yet. Pick a folder and add the job.</td></tr>
              )}
            </tbody>
          </table>
        </div>
        {(grid?.errors ?? []).length > 0 && (
          <ul className="error">{grid?.errors.map((item) => <li key={item}>{item}</li>)}</ul>
        )}
      </section>

      <section>
        <h2>4. Export or reply</h2>
        <button type="button" onClick={download}>Download Excel</button>
        <h3>Email draft</h3>
        <p className="muted">{mail}</p>
        <div className="row">
          <button type="button" className="secondary" onClick={() => invoke("connect_mail").then(refreshMail).catch((error: Error) => setMessage(plain(error)))}>Connect email</button>
          <button type="button" className="secondary" onClick={() => invoke("disconnect_mail").then(refreshMail).catch((error: Error) => setMessage(plain(error)))}>Disconnect email</button>
        </div>
        <label>Subject <input type="text" value={subject} onChange={(event) => setSubject(event.target.value)} /></label>
        <label>Message <textarea rows={4} value={draft} onChange={(event) => setDraft(event.target.value)} /></label>
        <button type="button" onClick={saveDrafts}>Save drafts for selected candidates</button>
        <p className="muted">Pick candidates in the list first. The draft is saved on this computer. This app does not send it.</p>
        <h3>Sample portal</h3>
        <p className="muted">Practice data on this computer. A live job site is not searched, and no portal password is stored.</p>
        <button type="button" className="secondary" onClick={findPortal}>Find sample profiles for this job</button>
        {portalNote && <p>{portalNote}</p>}
        {portalPeople.length > 0 && (
          <ul>
            {portalPeople.map((person) => (
              <li key={person.id}>
                <label>
                  <input
                    type="checkbox"
                    checked={portalPick.includes(person.id)}
                    onChange={() => setPortalPick((current) => current.includes(person.id) ? current.filter((item) => item !== person.id) : [...current, person.id])}
                  /> {person.name} · {person.location} · {person.notice} · {person.skills}
                </label>
              </li>
            ))}
          </ul>
        )}
        <button type="button" onClick={savePortal} disabled={portalPick.length === 0}>Save selected resumes to a folder</button>
      </section>
    </main>
  );
}

function rowKey(row: Row) {
  return `${row.rank}-${row.email}-${row.name}`;
}

function labelState(state: string) {
  if (state === "running") return "Reading";
  if (state === "paused") return "Paused";
  if (state === "cancelled") return "Cancelled";
  if (state === "done") return "Finished";
  return "Waiting";
}

function plain(error: unknown) {
  const text = String(error);
  return text.replace(/^Error:\s*/i, "");
}
