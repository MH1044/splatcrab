% covers: 4 - eig of a real matrix with a complex pair returns the complex eigenvalues, both of modulus 1
% Replaces 08's err_eig_complex_eigenvalues, the refusal of the same input: a
% rotation by 90 degrees, whose eigenvalues are i and -i.
e = eig([0 -1; 1 0]); disp(isreal(e)); fprintf('%.4f\n', abs(e))
