% covers: 6 - (Scope, dynamic fields) a struct given 100,000 fields one at a time by dynamic name keeps them all, in order, within the time limit
% Before the field index each new field walked every name before it, so this
% loop was quadratic and ran for minutes. fprintf avoids display widths.
s = struct(); for k = 1:100000, s.(sprintf('f%d', k)) = k; end
f = fieldnames(s); fprintf('%d %d\n', size(f)); disp(f{end}); fprintf('%d\n', s.f99999)
