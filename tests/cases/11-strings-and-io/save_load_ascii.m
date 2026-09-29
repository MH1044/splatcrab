% covers: 9 - (Scope, -ascii) save -ascii writes a matrix as text, and load -ascii reads it back as a double matrix
% Only the round trip is asserted, not the text's format: integers survive
% any digit count a text format could use. The file is named after the case
% and deleted before the case ends.
x = [1 2; 3 4];
save('save_load_ascii.txt', 'x', '-ascii');
y = load('save_load_ascii.txt', '-ascii');
disp(y)
disp(class(y))
delete('save_load_ascii.txt')
