% covers: 9 - save writes a double matrix, a char and a cell to a MAT file, and load after clear brings each of them back
% The file is named after the case and deleted before the case ends.
x = [1 2; 3 4];
s = 'hi';
c = {1, 'a'};
save('save_load_mat_round_trip.mat', 'x', 's', 'c');
clear;
load('save_load_mat_round_trip.mat');
disp(x)
disp(s)
disp(class(c))
delete('save_load_mat_round_trip.mat')
