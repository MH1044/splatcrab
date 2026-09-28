% covers: 6 - a rethrow that no try catches ends the script with the error's own message
% Item 6's inner try on its own. The error and the rethrow share one line,
% so the reported line is the same whichever of the two it names.
try, error('in'), catch e, rethrow(e), end
