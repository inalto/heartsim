"""Read a heartsim WFDB export with the reference `wfdb` package and check its shape and units.

Usage: python3 scripts/check_wfdb.py <record path without extension>
"""

import sys

import wfdb

LEADS = ["I", "II", "III", "aVR", "aVL", "aVF", "V1", "V2", "V3", "V4", "V5", "V6"]

record = wfdb.rdrecord(sys.argv[1])
assert record.n_sig == 12, record.n_sig
assert record.fs == 500, record.fs
assert record.sig_name == LEADS, record.sig_name
assert all(u == "mV" for u in record.units), record.units
assert record.sig_len >= 5000, record.sig_len
peak = abs(record.p_signal[:, 1]).max()
assert 0.3 < peak < 5.0, f"lead II peak {peak} mV"
print(f"{sys.argv[1]}: {record.sig_len} samples x {record.n_sig} leads at {record.fs} Hz, lead II peak {peak:.2f} mV")
