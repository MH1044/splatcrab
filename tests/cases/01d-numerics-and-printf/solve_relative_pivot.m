% covers: 9 - solve's pivot tolerance is relative to the matrix norm, and a genuinely singular matrix is still judged singular (a warning since cycle 08)
% The diagonal below is perfectly conditioned; only its scale tripped the old
% fixed 1e-14 threshold. The second solve is singular in any scaling. Since
% cycle 08 it warns on stderr instead of erroring (QA D26), so the .err holds
% the warning and the .exit is 0.
x = [1e-15 0; 0 1e-15] \ [1; 1];
fprintf('%g %g\n', x);
y = [1 2; 2 4] \ [1; 2];
