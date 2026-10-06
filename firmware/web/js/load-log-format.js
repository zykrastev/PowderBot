// Wall-clock timestamps are estimates: the ESP32 runs as an offline access point.
export function loadTimestamp(load, bootTime) {
    const milliseconds = bootTime + load.uptimeMs;
    return Number.isFinite(bootTime) && Number.isFinite(milliseconds)
        ? new Date(milliseconds).toISOString() : `Uptime ${load.uptimeMs} ms`;
}
export function loadValues(load, bootTime) {
    return [load.id, loadTimestamp(load, bootTime), load.powderName,
        load.targetWeight.toFixed(3), load.measuredWeight.toFixed(3),
        load.tolerance.toFixed(3), load.accepted ? "Accepted" : "Rejected"];
}
function csvCell(value) {
    let text = String(value);
    // Profile names are user input; prevent spreadsheet formula execution.
    if (/^\s*[=+@-]/.test(text)) text = `'${text}`;
    return `"${text.replaceAll('"', '""')}"`;
}
export function loadsCsv(loads, bootTime) {
    const rows = [["Load", "Timestamp (estimated, UTC)", "Powder", "Target (gr)",
        "Weighed (gr)", "Tolerance (gr)", "Result"],
        ...loads.map(load => loadValues(load, bootTime))];
    return '\uFEFF' + rows.map(row => row.map(csvCell).join(",")).join("\r\n") + "\r\n";
}
