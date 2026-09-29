% covers: 11 - (Design notes, error texts) close of a figure number that is not open is MATLAB's invalid handle error, exit 1
% Figure 1 is open, figure 7 is not. No file is written.
figure;
close(7);
