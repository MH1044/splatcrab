% covers: 5 - norm(v, p) for p = 1, 2, a positive real, Inf, -Inf, 'fro' and 'inf'
fprintf('%.4f %.4f %.4f %.4f %.4f %.4f %.4f\n', norm([3 -4], 1), norm([3 -4], 2), norm([3 -4], Inf), norm([3 -4], -Inf), norm([3 -4], 3), norm([3 -4], 'fro'), norm([3 -4], 'inf'));
fprintf('%.4f %.4f %.4f\n', norm([3; -4], 1), norm([1; -2; 2], Inf), norm([1; -2; 2], -Inf));
fprintf('%.4f %.4f %.4f\n', norm([1 2 3], 0.5), norm([1 2 3 4], 1.5), norm([2 0 -1], 4));
