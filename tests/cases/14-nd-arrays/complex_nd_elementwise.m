% covers: 8 - an N-D array times the imaginary unit keeps its shape and its imaginary parts, and adding a real N-D array keeps both parts
A = reshape(1:8, 2, 2, 2);
Z = A * 1i;
disp(size(Z))
disp(isreal(Z))
disp(imag(Z(:))')
disp(real(Z(:))')
W = Z + A;
disp(size(W))
disp(real(W(:))')
disp(imag(W(:))')
