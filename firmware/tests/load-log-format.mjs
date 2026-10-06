import assert from 'node:assert/strict';
import { loadValues, loadsCsv, loadTimestamp } from '../web/js/load-log-format.js';
const accepted = { id: 1, uptimeMs: 1500, powderName: 'Powder, "A"', targetWeight: 20,
    measuredWeight: 20.01, tolerance: 0.02, accepted: true };
const rejected = { ...accepted, id: 2, powderName: '=HYPERLINK("test")', measuredWeight: 20.1, accepted: false };
const boot = Date.parse('2026-10-06T10:00:00Z');
assert.deepEqual(loadValues(accepted, boot), [1, '2026-10-06T10:00:01.500Z', 'Powder, "A"', '20.000', '20.010', '0.020', 'Accepted']);
assert.equal(loadTimestamp(accepted, NaN), 'Uptime 1500 ms');
const csv = loadsCsv([accepted, rejected], boot);
assert.ok(csv.includes('"Powder, ""A"""'));
assert.ok(csv.includes('"\'=HYPERLINK(""test"")"'));
assert.ok(csv.includes('"20.100","0.020","Rejected"'));
assert.equal(csv.split('\r\n').length, 4);
console.log('Load log formatting: timestamp, precision, CSV escaping and formula protection passed.');
