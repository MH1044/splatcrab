% covers: 11 - (Scope, histc) histc with no edges counts nothing and returns an empty row for a row, exit 0, never an index past the edges
n = histc([1 2 3], []); disp(size(n))
