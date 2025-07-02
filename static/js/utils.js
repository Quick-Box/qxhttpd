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
                let s = `<div style="color: gray;"><del>${cell_val}</del></div>`;
                cell_val_chng.forEach(ch => {
                    s += `<div style="font-weight: bold; color: darkred;">${ch[0]}</div>
                          <div class="w3-small" style="color: darkblue;">${ch[1]}</div>`
                });
                row.cells[j].innerHTML = s;
            }
        }
    }
}

function fillTable(table, rows) {
    const tbody = table.tBodies[0]
    tbody.innerHTML = ""; // Removes all rows
    const start00 = table.dataset.start00;
    const header_row = table.querySelector(`thead tr`);
    const header_cells = header_row.querySelectorAll("th");
    for (const rec of rows) {
        const row = document.createElement("tr");
        for (const header_cell of header_cells) {
            const field_name = header_cell.dataset.fieldName;
            if (field_name === undefined) {
                continue;
            }
            const field_type = header_cell.dataset.fieldType;
            const cell = document.createElement("td");
            cell.className = header_cell.className;

            const rec_val = rec[field_name];
            let cell_val;
            if (field_name === "name") {
                cell_val = `${rec.last_name} ${rec.first_name}`;
            }
            else if (field_name === "time") {
                cell_val = obtime(msecSinceUntil(rec.start_time, rec.finish_time));
            }
            else if (field_type === "ObTime") {
                cell_val = obtime(msecSinceUntil(start00, rec_val));
            }
            else {
                if (rec_val === undefined) {
                    cell_val = '';
                }
                else {
                    cell_val = rec_val;
                }
            }
            cell.innerHTML = cell_val;
            row.appendChild(cell);
        }
        tbody.appendChild(row);
    }
}