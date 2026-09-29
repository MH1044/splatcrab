% covers: 5 - (Scope, fft ifft) a prime length, 100003, is fast and right: Parseval's identity, the sum of the bins and the round trip hold
% O(n log n) for every length is the Scope's requirement. A quadratic DFT at
% this length is 10^10 multiply-adds, far past the harness's 10-second timeout,
% so the case fails on time as well as on value. The checks are relative:
% loose enough for any sound algorithm, far too tight for a wrong one.
n = 100003; x = mod(7 * (1:n), 13); y = fft(x);
disp(abs(sum(abs(y) .^ 2) / (n * sum(x .^ 2)) - 1) < 1e-7)
disp(abs(sum(y) / (n * x(1)) - 1) < 1e-7)
disp(max(abs(ifft(y) - x)) < 1e-7)
