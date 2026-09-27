% covers: 6 - norm scales by the largest magnitude (QA D12), and a sum of no elements is +0, not -0
% NOTE: A comparison is a logical in MATLAB, and disp of a logical is four
% NOTE: characters wide ("   1"). SplatCrab has no logical class until cycle 02
% NOTE: and prints six wide. Adding 0 makes the value a double in both, so the
% NOTE: expected "     1" below is MATLAB's own output, and stays so after 02.
% NOTE: 1 / x tells +0 from -0: it is Inf for +0 and -Inf for -0.
fprintf('%g %g %g\n', norm([1e200 1e200]), norm([1e-200 1e-200]), norm([1e200 1e200], 3));
fprintf('%.4f %.4f %.4f\n', norm([]), sum([]), dot([], []));
fprintf('%g\n', 1 / sum([]));
fprintf('%g %g %g %g\n', 1 / norm([]), 1 / dot([], []), 1 / norm([0 0]), 1 / dot(zeros(1, 0), zeros(1, 0)));
fprintf('%g %g %g %g\n', norm([Inf 1]), norm([1 NaN]), norm([-Inf 2], 3), norm([1e-200 1e-200], 1));
disp(0 + (abs(norm([3e-200 4e-200]) / 5e-200 - 1) < 1e-12))
disp(0 + (abs(norm([3e200 4e200]) / 5e200 - 1) < 1e-12))
disp(0 + (abs(norm([1e-200 2e-200 2e-200], 3) / (17^(1/3) * 1e-200) - 1) < 1e-12))
