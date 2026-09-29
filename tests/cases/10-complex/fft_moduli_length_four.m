% covers: 5 - fft of [1 2 3 4]: the moduli of its four bins, 10, |-2+2i|, 2 and |-2-2i|
y = fft([1 2 3 4]); fprintf('%.4f ', abs(y)); fprintf('\n');
