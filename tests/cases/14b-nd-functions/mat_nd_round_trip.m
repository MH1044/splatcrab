% covers: 13 - save writes an N-D numeric, complex, logical and char array, in a variable, a cell and a struct field, with every dimension, a 1x1x1x2 and a 2x0x3 included, and load reads each back with its shape and class; the file is deleted before the case ends
A = reshape(1:24, 2, 3, 4);
L = A > 12;
c = 'ab';
c(:, :, 2) = 'cd';
q = {A};
s.f = A;
D = zeros(1, 1, 1, 2);
D(1, 1, 1, 2) = 7;
E = zeros(2, 0, 3);
Z = A - 1i * A;
try
  save('scratch_nd.mat', 'A', 'L', 'c', 'q', 's', 'D', 'E', 'Z');
  clear A L c q s D E Z
  load('scratch_nd.mat');
  failed = false;
catch e
  failed = true;
end
fid = fopen('scratch_nd.mat');
if fid >= 0
  fclose(fid);
  delete('scratch_nd.mat');
end
if failed
  rethrow(e)
end
B = reshape(1:24, 2, 3, 4);
disp(isequal(A, B))
disp(class(A))
disp(isequal(L, B > 12))
disp(class(L))
c2 = 'ab';
c2(:, :, 2) = 'cd';
disp(isequal(c, c2))
disp(class(c))
disp(size(c))
disp(c(:)')
disp(class(q))
disp(size(q))
x = q{1};
disp(class(x))
disp(size(x))
disp(x(:)')
disp(class(s.f))
disp(size(s.f))
y = s.f;
disp(y(:)')
disp(size(D))
disp(D(:)')
disp(class(E))
disp(size(E))
disp(isreal(Z))
disp(size(Z))
disp(isequal(Z, B - 1i * B))
