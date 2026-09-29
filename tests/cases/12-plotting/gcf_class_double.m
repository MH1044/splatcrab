% covers: 11 - (Scope, the gcf bullet) gcf returns the current figure's number as a double
% NOTE: since R2014b MATLAB's gcf returns a Figure object; SplatCrab has no
% NOTE: graphics objects and returns the number, as MATLAB did before
% NOTE: R2014b, the deviation the spec records. No file is written.
figure;
disp(class(gcf))
