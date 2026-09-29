% covers: 11 - (Scope, the lexer) the imaginary suffix on a decimal and on an exponent, 2.5j and 1e3i, and j is the unit until a variable takes the name
fprintf('%.4f %.4f\n', real(2.5j), imag(2.5j))
fprintf('%.4f %.4f\n', real(1e3i), imag(1e3i))
disp(imag(j)); j = 2; disp(j)
