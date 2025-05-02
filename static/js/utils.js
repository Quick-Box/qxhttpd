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
function formatRunTable(changes) {
    const table = document.getElementById('table');
    const start00 = table.dataset.start00;
    const headerRow = table.querySelector(`thead tr`);
    const headerCells = headerRow.querySelectorAll("th");
    for (let j = 0; j < headerCells.length; j++) {
        const col_name = headerCells[j].dataset.colName;
        if (col_name === undefined) {
            continue;
        }
        for (let i = 1; i < table.rows.length; i++) {
            const row = table.rows[i];
            let cell_val;
            if (col_name === "start_time") {
                cell_val = obtime(msecSinceUntil(start00, row.dataset.start_time));
            }
            else if (col_name === "finish_time") {
                cell_val = obtime(msecSinceUntil(start00, row.dataset.finish_time));
            }
            else if (col_name === "time") {
                cell_val = obtime(msecSinceUntil(row.dataset.start_time, row.dataset.finish_time));
            }
            else {
                const val = row.dataset[col_name];
                // console.log(i, j, col_name, val);
                cell_val = `${val}`;
            }
            let cell_val_chng = [];
            if (col_name !== "run_id") {
                const run_id= Number(row.dataset.run_id);
                changes.filter(ch => {
                    return ch.run_id === run_id
                }).forEach(ch => {
                    let ch2 = ch.data.RunUpdateRequest[col_name];
                    if (ch2 !== undefined) {
                        cell_val_chng.push([ch2, ch.user_id]);
                    }
                });
            }
            if (cell_val_chng.length === 0) {
                row.cells[j].innerHTML = cell_val;
            }
            else {
                let s = `<span style="color: gray;"><del>${cell_val}</del></span>`;
                cell_val_chng.forEach(ch => {
                    s += `<br><span style="font-weight: bold; color: darkred;">${ch[0]}</span>
                          <br><span class="w3-small" style="color: darkblue;">${ch[1]}</span>`
                });
                row.cells[j].innerHTML = s;
            }
        }
    }
}