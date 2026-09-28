% covers: 3 - [Q, R] = qr(A) is a Householder factorisation whose product gives A back
% R(1, 1) is norm(A(:, 1)) = sqrt(10) up to the sign the reflector chooses,
% so the case takes abs; the residual prints 0.0000 at four decimals
% whatever its last digits.
[Q, R] = qr([1 2; 3 4]); fprintf('%.4f\n', abs(R(1, 1))); fprintf('%.4f\n', norm(Q * R - [1 2; 3 4]))
