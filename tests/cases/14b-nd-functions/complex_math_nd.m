% covers: 5 - a complex N-D array through real, imag, conj and abs keeps its shape, real and imag giving real arrays and conj keeping the complex storage, and sqrt of a real N-D array with negative elements is a complex array of its shape (self-checked)
Z = reshape([3 5 -8 0], 1, 2, 2) + 1i * reshape([4 -12 6 1], 1, 2, 2);
R = real(Z);
disp(size(R))
disp(isreal(R))
disp(R(:)')
J = imag(Z);
disp(size(J))
disp(isreal(J))
disp(J(:)')
C = conj(Z);
disp(size(C))
disp(isreal(C))
K = imag(C);
disp(K(:)')
disp(isequal(real(C), R))
B = abs(Z);
disp(size(B))
disp(B(:)')
S = sqrt(-(reshape(1:8, 2, 2, 2) .^ 2));
disp(size(S))
disp(isreal(S))
T = imag(S);
disp(all(abs(T(:)' - (1:8)) < 1e-12))
disp(all(real(S(:)) == 0))
