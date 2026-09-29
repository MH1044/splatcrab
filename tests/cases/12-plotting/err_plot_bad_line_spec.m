% covers: 8 - (Design notes, error texts) text that is not a line spec is refused with the text it was given, exit 1
% NOTE: MATLAB would read 'LineWidth', 2 as a name-value option; SplatCrab
% NOTE: takes none and refuses the name, the deviation the spec records.
% No file is written.
plot(1:3, 'LineWidth', 2);
