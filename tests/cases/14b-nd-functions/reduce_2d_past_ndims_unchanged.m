% covers: 1 - a reduction of a matrix along a dimension past ndims gives today's answer, values and storage alike: a -0 stays -0 past ndims, and sum keeps it along a dimension of size 1 within ndims too, as the sum page settles, and prod keeps a complex storage that sum drops, as before
y = -0;
disp(1/sum(y, 3))
disp(1/mean(y, 3))
disp(1/cumsum(y, 3))
disp(1/sum(y, 1))
z = complex(1, 0);
disp(isreal(prod(z, 3)))
disp(isreal(sum(z, 3)))
