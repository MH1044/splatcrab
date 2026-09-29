% covers: 6 - (Scope, Matrix.im) growing a complex array in a loop is linear, as a real one is:
% the imaginary parts are scattered in place (cycle 10's review found every write copying and
% rescanning the whole array, which took 62 s for 160,000 appends)
z = []; for k = 1:200000, z(end+1) = k * 1i; end
disp(numel(z))
disp(imag(z(end)) == 200000)
