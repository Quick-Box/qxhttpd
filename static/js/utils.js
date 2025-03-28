function msecSinceUntil(since_str, until_str) {
    const since_msec = Date.parse(since_str);
    const until_msec = Date.parse(until_str);
    if (isNaN(since_msec) || isNaN(until_msec)) {
        return null;
    }
    return until_msec - since_msec;
}

function obtime(msec) {
    if (typeof msec === 'number') {
        let sec = msec / 1000 | 0;
        let min = sec / 60 | 0;
        sec = sec % 60;
        return `${min}:${sec.toString().padStart(2, '0')}`;
    }
    return '';
}

function findColumnIndex(table, col_name) {
    const headerRow = table.querySelector(`thead tr`);
    const headerCells = headerRow.querySelectorAll("th");
    for (let i = 0; i < headerCells.length; i++) {
        if (headerCells[i].classList.contains(col_name)) {
            return i;
        }
    }
    return -1;
}
function forEachTableRow(table, format_fn) {
    for (let i = 1; i < table.rows.length; i++) {
        const row = table.rows[i];
        format_fn(row);
    }
}

function formatRunTable(start00) {
    const table = document.getElementById('tables');
    let col_start_time = findColumnIndex(table, 'start_time');
    if (col_start_time >= 0) {
        let col_finish_time = findColumnIndex(table, 'finish_time');
        forEachTableRow(table, (row) => {
            let start_time = row.cells[col_start_time].innerHTML;
            row.cells[col_start_time].innerHTML = obtime(msecSinceUntil(start00, start_time));
            if (col_finish_time >= 0) {
                let finish_time = row.cells[col_finish_time].innerHTML;
                row.cells[col_finish_time].innerHTML = obtime(msecSinceUntil(start00, finish_time));
                let col_time = findColumnIndex(table, 'time');
                if (col_time >= 0) {
                    row.cells[col_time].innerHTML = obtime(msecSinceUntil(start_time, finish_time));
                }
            }
        });
    }
}