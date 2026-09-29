% covers: 5 - fft of a length-4 impulse is flat: every bin has modulus 1
fprintf('%.4f ', abs(fft([1 0 0 0]))); fprintf('\n');
