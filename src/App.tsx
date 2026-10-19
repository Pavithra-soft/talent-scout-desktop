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
  const filtersRef = useRef(filters);
  filtersRef.current = filters;

  async function refresh(next?: Filters) {
    const result = await invoke<Grid>("list_results", { filters: next ?? filtersRef.current });
    setGrid(result);
  }

  useEffect(() => {
    const unlisten = listen<Progress>("ingest-progress", (event) => {
      setProgress(event.payload);
      if (event.payload.state === "done" || event.payload.state === "cancelled") {
        setBusy(false);
        refresh().catch((error: Error) => setMessage(error.message));
      }
    });
    invoke<string>("sample_jd").then(setJd).catch(() => undefined);
    refresh().catch(() => undefined);
    const dropped = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "drop" && event.payload.paths[0]) {
        begin(event.payload.paths[0]).catch((error: Error) => setMessage(String(error)));
      }
    });
    return () => {
      unlisten.then((stop) => stop());
      dropped.then((stop) => stop());
    };
  }, []);

  async function chooseFolder() {
    const selected = await open({ directory: true, multiple: false, title: "Choose the resume folder" });
    if (typeof selected === "string") {
      await begin(selected);
    }
  }

  async function begin(path: string) {
    setBusy(true);
    setMessage("");
    try {
      await invoke("ingest_start", { path });
    } catch (error) {
      setBusy(false);
      setMessage(String(error));
    }
  }

  async function rank() {
    setMessage("");
    try {
      const result = await invoke<Grid>("rank_profiles", { jdText: jd, weights });
      setGrid(result);
      setMessage(`Ranked ${result.rows.length} profiles on this computer.`);
    } catch (error) {
      setMessage(String(error));
    }
  }

  async function applyFilters() {
    try {
      await refresh();
    } catch (error) {
      setMessage(String(error));
    }
  }

  async function download() {
    const dest = await save({
      defaultPath: "ranked-profiles.xlsx",
      filters: [{ name: "Excel", extensions: ["xlsx"] }],
    });
    if (typeof dest !== "string") return;
    try {
      const count = await invoke<number>("export_excel", { path: dest, filters });
      setMessage(`Saved ${count} rows to Excel.`);
    } catch (error) {
      setMessage(String(error));
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

  const percent = progress && progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : 0;

  return (
    <main
      onDragOver={(event) => event.preventDefault()}
      onDrop={(event) => event.preventDefault()}
    >
      <header>
        <h1>Talent Scout Desktop</h1>
        <p>Resumes stay on this computer. Nothing is uploaded. No extra software is required.</p>
      </header>

      <section>
        <h2>1. Read resumes</h2>
        <div className="row">
          <button type="button" onClick={chooseFolder} disabled={busy}>Choose resume folder</button>
          <button
            type="button"
            className="secondary"
            disabled={busy}
            onClick={() => {
              setBusy(true);
              setMessage("");
              invoke("load_samples").catch((error: Error) => {
                setBusy(false);
                setMessage(String(error));
              });
            }}
          >
            Try the sample resumes
          </button>
          <button type="button" className="secondary" disabled={!busy} onClick={() => invoke("ingest_pause")}>Pause</button>
          <button type="button" className="secondary" disabled={!busy} onClick={() => invoke("ingest_resume")}>Resume</button>
          <button type="button" className="secondary" disabled={!busy} onClick={() => invoke("ingest_cancel")}>Cancel</button>
        </div>
        <div className="progress" aria-hidden="true"><span style={{ width: `${percent}%` }} /></div>
        <p className="muted">
          {progress
            ? `${progress.state}: ${progress.done} of ${progress.total}. New files ${progress.parsed}, unchanged ${progress.skipped}, unreadable ${progress.errors}. ${Math.round(progress.files_per_min)} files per minute.`
            : "Choose a folder, or drop one onto this window. PDF, Word, RTF, and text files are read. A second pass skips files that have not changed."}
        </p>
        {progress && progress.messages.length > 0 && (
          <ul className="error">
            {progress.messages.map((item) => <li key={item}>{item}</li>)}
          </ul>
        )}
      </section>

      <section>
        <h2>2. Rank against the job</h2>
        <label>Job description
          <textarea rows={8} value={jd} onChange={(event) => setJd(event.target.value)} />
        </label>
        <div className="weights">
          <label>Must-have weight <input type="number" value={weights.must} onChange={(event) => setWeights({ ...weights, must: Number(event.target.value) })} /></label>
          <label>Nice-to-have weight <input type="number" value={weights.nice} onChange={(event) => setWeights({ ...weights, nice: Number(event.target.value) })} /></label>
          <label>Experience weight <input type="number" value={weights.experience} onChange={(event) => setWeights({ ...weights, experience: Number(event.target.value) })} /></label>
          <label>Location weight <input type="number" value={weights.location} onChange={(event) => setWeights({ ...weights, location: Number(event.target.value) })} /></label>
        </div>
        <button type="button" onClick={rank}>Rank profiles</button>
        <p className="muted">Ranking uses rules on this computer. JS counts as JavaScript. k8s counts as Kubernetes. Blank fields stay blank.</p>
      </section>

      <section>
        <h2>3. Filter and download Excel</h2>
        <div className="row">
          <label>Notice
            <select value={filters.notice} onChange={(event) => setFilters({ ...filters, notice: event.target.value })}>
              <option value="any">Any</option>
              <option value="immediate">Immediate or serving notice</option>
              <option value="15">15 days</option>
              <option value="30">30 days</option>
              <option value="60">60 days</option>
              <option value="90">90 days</option>
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
          <label>Minimum score <input type="number" value={filters.score_min ?? ""} onChange={(event) => setFilters({ ...filters, score_min: event.target.value === "" ? null : Number(event.target.value) })} /></label>
        </div>
        <p>Location</p>
        <div className="row">
          {(grid?.locations ?? []).map((city) => (
            <label key={city}>
              <input type="checkbox" checked={filters.locations.includes(city)} onChange={() => toggleCity(city)} /> {city}
            </label>
          ))}
          {(grid?.locations ?? []).length === 0 && <span className="muted">Locations appear after resumes are read.</span>}
        </div>
        <div className="row">
          <button type="button" className="secondary" onClick={applyFilters}>Apply filters</button>
          <button type="button" onClick={download}>Download Excel</button>
        </div>
        {message && <p>{message}</p>}
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Rank</th><th>Name</th><th>Email</th><th>Phone</th><th>Location</th><th>Skill Set</th><th>Matching Skills</th><th>Non-Matching Skills</th><th>Previous Company</th><th>Notice Period</th><th>Score</th>
              </tr>
            </thead>
            <tbody>
              {(grid?.rows ?? []).map((row) => (
                <tr key={`${row.rank}-${row.email}-${row.name}`} title={row.rationale}>
                  <td>{row.rank}</td>
                  <td>{row.name}</td>
                  <td>{row.email}</td>
                  <td>{row.phone}</td>
                  <td>{row.location}</td>
                  <td>{row.skills}</td>
                  <td>{row.matching}</td>
                  <td>{row.missing}</td>
                  <td>{row.company}</td>
                  <td>{row.notice}</td>
                  <td>{row.score ?? ""}</td>
                </tr>
              ))}
              {(grid?.rows ?? []).length === 0 && (
                <tr><td colSpan={11}>No profiles yet.</td></tr>
              )}
            </tbody>
          </table>
        </div>
        {(grid?.errors ?? []).length > 0 && (
          <ul className="error">
            {grid?.errors.map((item) => <li key={item}>{item}</li>)}
          </ul>
        )}
      </section>
    </main>
  );
}
