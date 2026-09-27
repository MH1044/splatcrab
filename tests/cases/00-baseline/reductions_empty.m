% covers: reductions on empty matrices keep MATLAB's identity values and shapes
fprintf('%g\n', sum([]));
fprintf('%g\n', prod([]));
fprintf('%g\n', mean([]));
fprintf('%g\n', any([]));
fprintf('%g\n', all([]));
fprintf('%d %d\n', isempty(max([])), isempty(min([])));
disp(sum(zeros(0, 3)))
disp(prod(zeros(0, 3)))
fprintf('%g %g %g\n', mean(zeros(0, 3)));
fprintf('%d %d\n', size(sum(zeros(0, 3))));
fprintf('%d %d\n', size(sum(zeros(3, 0))));
fprintf('%g %g %g\n', norm([]), det([]), trace([]));
