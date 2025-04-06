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
/*
function findColumnIndex(table, col_name) {
    const headerRow = table.querySelector(`thead tr`);
    const headerCells = headerRow.querySelectorAll("th");
    for (let i = 0; i < headerCells.length; i++) {
        if (headerCells[i].dataset.colName === col_name) {
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
*/
function formatRunTable(start00) {
    const table = document.getElementById('table');
    const headerRow = table.querySelector(`thead tr`);
    const headerCells = headerRow.querySelectorAll("th");
    for (let j = 0; j < headerCells.length; j++) {
        const col_name = headerCells[j].dataset.colName;
        for (let i = 1; i < table.rows.length; i++) {
            const row = table.rows[i];
            if (col_name === "start_time") {
                row.cells[j].innerHTML = obtime(msecSinceUntil(start00, row.dataset.start_time));
            }
            else if (col_name === "finish_time") {
                row.cells[j].innerHTML = obtime(msecSinceUntil(start00, row.dataset.finish_time));
            }
            else if (col_name === "time") {
                row.cells[j].innerHTML = obtime(msecSinceUntil(row.dataset.start_time, row.dataset.finish_time));
            }
            else if (col_name === "name") {
                row.cells[j].innerHTML = `${row.dataset.last_name} ${row.dataset.first_name}`;
            }
            else {
                const val = row.dataset[col_name];
                // console.log(i, j, col_name, val);
                row.cells[j].innerHTML = `${val}`;
            }
        }
    }
}