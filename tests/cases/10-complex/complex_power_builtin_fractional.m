% covers: 10 - the same principal root through the power builtin
% Replaces 01d's err_complex_power_builtin. The first line is the spec's,
% power(-8, 1/3); the second is the refusal's own input, power(-2, 0.5), the
% principal square root, checked by its distance from sqrt(2) * 1i.
z = power(-8, 1/3); fprintf('%.4f %.4f\n', real(z), imag(z))
disp(abs(power(-2, 0.5) - sqrt(2) * 1i) < 1e-12)
