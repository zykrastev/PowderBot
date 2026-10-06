async function request(path, method = "GET", body) {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 6000);
    try {
        const response = await fetch(path, {
            method, signal: controller.signal, cache: "no-store",
            headers: body === undefined ? {} : { "Content-Type": "application/json" },
            body: body === undefined ? undefined : JSON.stringify(body)
        });
        const data = await response.json();
        if (!response.ok) throw new Error(data.error || `Request failed (${response.status})`);
        if (response.headers.has("X-Profile-Warnings") && path === "/api/powders" && method === "GET") {
            window.dispatchEvent(new CustomEvent("profile-warnings", { detail: Number(response.headers.get("X-Profile-Warnings")) }));
        }
        return data;
    } catch (error) {
        if (error.name === "AbortError") throw new Error("Request timed out. Reload to check the result before retrying a change.");
        throw error;
    } finally { clearTimeout(timeout); }
}
export default {
    getStatus: () => request("/api/status"),
    getLoads: () => request("/api/loads"),
    clearLoads: () => request("/api/loads", "DELETE"),
    start: () => request("/api/start", "POST"),
    stop: () => request(`http://${location.hostname}:81/api/stop`, "POST"),
    tare: () => request("/api/tare", "POST"),
    reset: () => request("/api/reset", "POST"),
    getSystemInfo: () => request("/api/system"),
    listPowders: () => request("/api/powders"),
    getActivePowder: () => request("/api/active-powder"),
    setActivePowder: storageName => request("/api/active-powder", "PUT", { storageName }),
    createPowder: profile => request("/api/powders", "POST", profile),
    updatePowder: profile => request("/api/powders", "PUT", profile),
    deletePowder: name => request(`/api/powders?name=${encodeURIComponent(name)}`, "DELETE"),
    setTargetWeight: targetWeight => request("/api/settings", "PUT", { targetWeight }),
    setTolerance: tolerance => request("/api/settings", "PUT", { tolerance })
};
