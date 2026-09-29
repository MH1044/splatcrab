% covers: 13 - a cell and a struct field hold a complex value intact
c = {1+2i}; s.z = 3i; disp(imag(c{1}) + imag(s.z))
