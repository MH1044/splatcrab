% covers: 7 - (Scope, the complex flag) an indexed assignment keeps complex storage complex, even
% when every imaginary part it leaves is zero; the next arithmetic result drops it (Design notes)
z = [1i 2i]; z(1) = 3; z(2) = 4; disp(isreal(z))
w = z + 0; disp(isreal(w))
