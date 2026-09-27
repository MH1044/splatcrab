% covers: 8 - matmul no longer skips a zero factor, so Inf * 0 and NaN * 0 happen and propagate
% The `if b == 0.0 { continue }` sparsity shortcut never performed the
% multiply, so the non-finite factor vanished and the sum stayed 0.
% NOTE: asserted through fprintf('%g') rather than the spec's disp, because a
% non-finite element still forces the whole row to the four-decimal display
% width. That row is scheduled to 01e; give this case its disp lines back then.
fprintf('%g\n', [Inf 0] * [0; 1]);
fprintf('%g\n', [NaN 0] * [0; 1]);
