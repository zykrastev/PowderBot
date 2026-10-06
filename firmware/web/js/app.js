import Api from "./Api.js";
import "./profiles.js";

const byId = id => document.getElementById(id);
const select = byId("dashboardProfileSelect");
const target = byId("targetWeight");
const tolerance = byId("tolerance");
const notice = byId("apiNotice");
let running = false;
let actionPending = false;
let deviceReady = false;
let canStart = false;
const saving = new Set();
function controls() {
    const start = byId("startButton");
    start.disabled = actionPending || (!running && (!deviceReady || !canStart));
    start.title = running ? "Stop dispensing" : canStart ? "Start load" : "Empty and tare the scale before starting";
    byId("startButtonText").textContent = running ? "STOP" : "START";
    byId("startButtonIcon").src = running ? "img/icons/square.svg" : "img/icons/play.svg";
    start.classList.toggle("running", running);
    byId("tareButton").disabled = actionPending || !deviceReady;
    byId("resetButton").disabled = actionPending || !deviceReady;
    target.disabled = running || actionPending || saving.has(target);
    tolerance.disabled = running || actionPending || saving.has(tolerance);
    select.disabled = running || actionPending || saving.has(select);
}
async function command(action) {
    if (actionPending) return;
    actionPending = true; controls();
    try { await action(); notice.textContent = ""; }
    catch (error) { report(error); }
    finally { actionPending = false; controls(); }
}
byId("startButton").addEventListener("click", () => command(running ? Api.stop : Api.start));
byId("tareButton").addEventListener("click", () => command(Api.tare));
byId("resetButton").addEventListener("click", () => command(Api.reset));
const network = document.querySelector(".network-status");
document.querySelector(".network-ip").textContent = location.hostname;
window.addEventListener("profile-warnings", event => {
    byId("profileWarning").textContent = event.detail > 0 ? "Some profiles could not be listed. See the log console for details." : "";
});
function report(error) { notice.textContent = error.message; }
async function loadProfiles() {
    try {
        const [profiles, active] = await Promise.all([Api.listPowders(), Api.getActivePowder()]);
        select.replaceChildren(new Option("No profile selected", ""));
        for (const profile of profiles) select.add(new Option(profile.name, profile.storageName));
        select.value = active?.storageName || "";
    } catch (error) { report(error); }
}
select.addEventListener("change", async () => {
    if (!select.value) { await loadProfiles(); return; }
    saving.add(select); controls();
    try { await Api.setActivePowder(select.value); notice.textContent = ""; }
    catch (error) { report(error); await loadProfiles(); }
    finally { saving.delete(select); controls(); }
});
async function showPage(name) {
    document.querySelectorAll(".page").forEach(page => { page.hidden = page.id !== `${name}Page`; });
    document.querySelectorAll(".nav-item").forEach(button => button.classList.toggle("active", button.dataset.page === name));
    if (name === "dashboard") await loadProfiles();
    if (name === "system") {
        try {
            const info = await Api.getSystemInfo();
            byId("systemProductName").textContent = info.name;
            byId("systemVersion").textContent = info.version;
            byId("systemAuthor").textContent = info.author;
        } catch (error) { report(error); }
    }
}
document.querySelectorAll(".nav-item").forEach(button => button.addEventListener("click", () => showPage(button.dataset.page)));
byId("manageProfilesButton").addEventListener("click", () => showPage("profiles"));
for (const [input, save] of [[target, Api.setTargetWeight], [tolerance, Api.setTolerance]]) {
    input.addEventListener("keydown", event => { if (event.key === "Enter") input.blur(); });
    input.addEventListener("change", async () => {
        const value = Number(input.value);
        if (!Number.isFinite(value) || value <= 0 || value > 1000) { report(new Error("Enter a value greater than 0 and at most 1000 grains.")); return; }
        saving.add(input); controls();
        try { await save(value); notice.textContent = ""; }
        catch (error) { report(error); }
        finally { saving.delete(input); controls(); }
    });
}
function clearWeight(message) {
    document.querySelector(".status").classList.remove("ready");
    byId("currentWeight").textContent = "—";
    byId("remainingWeight").textContent = "—";
    byId("statusText").textContent = message;
    byId("progressFill").style.width = "0%";
    byId("progressBar").setAttribute("aria-valuenow", "0");
    byId("progressPercent").textContent = "—";
    byId("stabilityText").textContent = "Unavailable";
    byId("stabilityIndicator").classList.add("is-unstable");
}
async function poll() {
    try {
        const status = await Api.getStatus();
        network.textContent = "Connected";
        running = Boolean(status.dispensing);
        deviceReady = Boolean(status.capabilities?.dispensing);
        canStart = Boolean(status.canStart);
        byId("startRequirement").textContent = running || canStart ? "" :
            !status.scaleConnected ? "Waiting for a fresh scale reading." :
            status.targetWeight <= 0 ? "Set a target weight before starting." :
            `Empty and tare the scale: Start requires 0 ± ${status.tolerance} gr.`;
        controls();
        updateLoadLog(status);
        const rejected = Boolean(status.overthrowAlert);
        byId("overthrowAlert").hidden = !rejected;
        document.querySelector(".status").classList.toggle("is-rejected", rejected);
        document.querySelector(".weight-card").classList.toggle("is-rejected", rejected);
        window.dispatchEvent(new CustomEvent("controller-state", { detail: running }));
        if (document.activeElement !== target && !target.disabled) target.value = status.targetWeight.toFixed(2);
        if (document.activeElement !== tolerance && !tolerance.disabled) tolerance.value = status.tolerance.toFixed(2);
        byId("progressTarget").textContent = `${status.targetWeight.toFixed(2)} gr`;
        if (status.scaleConnected && Number.isFinite(status.currentWeight)) {
            document.querySelector(".status").classList.add("ready");
            byId("currentWeight").textContent = status.currentWeight.toFixed(2);
            byId("remainingWeight").textContent = status.remainingWeight.toFixed(2);
            byId("statusText").textContent = status.controllerError ? `${status.state}: ${status.controllerError}` : status.state;
            byId("stabilityText").textContent = status.stable ? "Stable" : "Unstable";
            byId("stabilityIndicator").classList.toggle("is-unstable", !status.stable);
            byId("stabilityIcon").src = status.stable ? "img/icons/circle-check.svg" : "img/icons/circle-alert.svg";
            const percent = status.targetWeight > 0 ? Math.max(0, Math.min(100, 100 * status.currentWeight / status.targetWeight)) : 0;
            byId("progressFill").style.width = `${percent}%`;
            byId("progressBar").setAttribute("aria-valuenow", percent.toFixed(1));
            byId("progressPercent").textContent = `${percent.toFixed(1)}%`;
        } else { clearWeight(status.scaleError || "Scale unavailable"); }
    } catch (error) { deviceReady = false; canStart = false; controls(); byId("startRequirement").textContent = "Waiting for the device."; updateLoadLog(null); network.textContent = "Disconnected / retrying"; clearWeight("Device unavailable"); }
    finally { setTimeout(poll, 250); }
}
loadProfiles();
poll();
