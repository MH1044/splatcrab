% covers: 5 - ifft undoes fft at length 3, which is not a power of two, to four decimals
% The round trip is not exact, so it is printed with %.4f rather than displayed.
fprintf('%.4f ', real(ifft(fft([1 2 3])))); fprintf('\n')
