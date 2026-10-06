import Api from "./Api.js";
import { loadValues, loadsCsv } from "./load-log-format.js";
const byId = id => document.getElementById(id);
let loads = [];
let revision = null;
let bootTime = NaN;
let lastUptime = null;
let lastObservedAt = null;
let connected = false;
let running = false;
let pending = false;
let refreshing = false;
function controls() {
    for (const id of ["downloadLoads", "printLoads"]) {
        byId(id).disabled = !connected || pending || refreshing || loads.length === 0;
    }
    byId("clearLoads").disabled = !connected || running || pending || refreshing || loads.length === 0;
}
function render() {
    const rows = loads.map(load => {
        const row = document.createElement("tr");
        row.className = load.accepted ? "load-accepted" : "load-rejected";
        const values = loadValues(load, bootTime);
        if (Number.isFinite(bootTime)) values[1] = new Date(bootTime + load.uptimeMs).toLocaleString();
        for (const value of values) {
            const cell = document.createElement("td");
            cell.textContent = value;
            row.append(cell);
        }
        return row;
    });
    byId("loadLogRows").replaceChildren(...rows);
    byId("loadLogStatus").textContent = loads.length ? `${loads.length} completed load${loads.length === 1 ? "" : "s"}` : "No completed loads in this series.";
    controls();
}
async function refresh() {
    if (refreshing) return;
    refreshing = true; controls();
    try {
        const started = performance.now();
        const history = await Api.getLoads();
        bootTime = Date.now() - (performance.now() - started) / 2 - history.uptimeMs;
        loads = history.loads;
        revision = history.revision;
        render();
    } catch (error) {
        revision = null;
        byId("loadLogStatus").textContent = `History unavailable: ${error.message}`;
        connected = false;
    } finally { refreshing = false; controls(); }
}
export function updateLoadLog(status) {
    connected = Boolean(status);
    running = Boolean(status?.dispensing);
    if (!status) {
        revision = null;
        byId("loadLogStatus").textContent = "Device unavailable — reconnecting to load history.";
    } else {
        const observedAt = Date.now();
        if (lastUptime !== null && (status.uptimeMs < lastUptime ||
            Math.abs((observedAt - lastObservedAt) - (status.uptimeMs - lastUptime)) > 2000)) {
            bootTime = NaN; revision = null; loads = [];
            byId("loadLogRows").replaceChildren();
        }
        lastUptime = status.uptimeMs;
        lastObservedAt = observedAt;
        if (revision !== status.loadRevision && !pending) void refresh();
    }
    controls();
}
byId("downloadLoads").addEventListener("click", () => {
    const blob = new Blob([loadsCsv(loads, bootTime)], { type: "text/csv;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `powderbot-loads-${new Date().toISOString().replaceAll(":", "-")}.csv`;
    document.body.append(link); link.click(); link.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
});
byId("printLoads").addEventListener("click", () => window.print());
byId("clearLoads").addEventListener("click", async () => {
    if (pending || running) return;
    pending = true; controls();
    try { await Api.clearLoads(); await refresh(); }
    catch (error) { byId("loadLogStatus").textContent = `Could not reset log: ${error.message}`; }
    finally { pending = false; controls(); }
});
