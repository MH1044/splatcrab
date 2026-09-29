% covers: 10 - writematrix writes a matrix as comma-separated text, readmatrix reads it back, and fileread returns the whole text
% The file is named after the case and deleted before the case ends.
writematrix([1 2; 3 4], 'writematrix_readmatrix_fileread.csv');
disp(readmatrix('writematrix_readmatrix_fileread.csv'))
disp(fileread('writematrix_readmatrix_fileread.csv'))
delete('writematrix_readmatrix_fileread.csv')
