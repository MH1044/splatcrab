% covers: 9 - filter with a recursive denominator: the impulse response of y(n) = x(n) + 0.5 y(n-1)
fprintf('%.2f ', filter(1, [1 -0.5], [1 0 0])); fprintf('\n');
