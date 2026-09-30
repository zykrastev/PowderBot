import Api from "./Api.js";
import "./profiles.js";

const byId = id => document.getElementById(id);
const select = byId("dashboardProfileSelect");
const target = byId("targetWeight");
const tolerance = byId("tolerance");
const notice = byId("apiNotice");
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
    select.disabled = true;
    try { await Api.setActivePowder(select.value); notice.textContent = ""; }
    catch (error) { report(error); await loadProfiles(); }
    finally { select.disabled = false; }
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
        input.disabled = true;
        try { await save(value); notice.textContent = ""; }
        catch (error) { report(error); }
        finally { input.disabled = false; }
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
}
async function poll() {
    try {
        const status = await Api.getStatus();
        network.textContent = "Connected";
        if (document.activeElement !== target && !target.disabled) target.value = status.targetWeight.toFixed(2);
        if (document.activeElement !== tolerance && !tolerance.disabled) tolerance.value = status.tolerance.toFixed(2);
        byId("progressTarget").textContent = `${status.targetWeight.toFixed(2)} gr`;
        if (status.scaleConnected && Number.isFinite(status.currentWeight)) {
            document.querySelector(".status").classList.add("ready");
            byId("currentWeight").textContent = status.currentWeight.toFixed(2);
            byId("remainingWeight").textContent = status.remainingWeight.toFixed(2);
            byId("statusText").textContent = "Scale connected";
            byId("stabilityText").textContent = "Stability not measured";
            const percent = status.targetWeight > 0 ? Math.max(0, Math.min(100, 100 * status.currentWeight / status.targetWeight)) : 0;
            byId("progressFill").style.width = `${percent}%`;
            byId("progressBar").setAttribute("aria-valuenow", percent.toFixed(1));
            byId("progressPercent").textContent = `${percent.toFixed(1)}%`;
        } else { clearWeight(status.scaleError || "Scale unavailable"); }
    } catch (error) { network.textContent = "Disconnected / retrying"; clearWeight("Device unavailable"); }
    finally { setTimeout(poll, 1000); }
}
loadProfiles();
poll();
