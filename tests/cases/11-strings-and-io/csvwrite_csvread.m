% covers: 10 - (Scope, csvwrite and csvread) csvwrite writes a matrix as comma-separated text and csvread reads it back
% The file is named after the case and deleted before the case ends.
csvwrite('csvwrite_csvread.csv', [1 2; 3 4]);
disp(csvread('csvwrite_csvread.csv'))
disp(fileread('csvwrite_csvread.csv'))
delete('csvwrite_csvread.csv')
