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


function fillTable(table, rows) {
    const tbody = table.tBodies[0]
    tbody.innerHTML = ""; // Removes all rows
    const start00 = table.dataset.start00;
    const header_cells = table.querySelectorAll("thead th");
    let row_no = 0;
    for (const rec of rows) {
        row_no++;
        const row = document.createElement("tr");
        for (const header_cell of header_cells) {
            const cell = document.createElement("td");
            const field_name = header_cell.dataset.fieldName;
            const field_type = header_cell.dataset.fieldType;
            cell.className = header_cell.className;

            if (field_name === "name") {
                cell.innerHTML = `${rec.last_name} ${rec.first_name}`;
            }
            else if (field_name === "time") {
                cell.innerHTML = obtime(msecSinceUntil(rec.start_time, rec.finish_time));
            }
            else if (field_type === "RowNumber") {
                cell.innerHTML = `${row_no}.`
            }
            else if (field_type === "EditRow") {
                const fn_name = header_cell.dataset.fnName;
                const id_field_name = header_cell.dataset.idFieldName;
                const row_id = rec[id_field_name];
                cell.innerHTML = `<i onClick="${fn_name}(${row_id})" class="w3-button w3-round w3-theme fa fa-pencil"></i>`
            }
            else if (field_type === "RelativeToStartObTime") {
                const rec_val = rec[field_name];
                cell.innerHTML = obtime(msecSinceUntil(start00, rec_val));
            }
            else {
                if (field_name === undefined) {
                    cell.innerHTML = "Field name missing";
                }
                else {
                    const rec_val = rec[field_name];
                    if (rec_val === undefined) {
                        cell.innerHTML = '';
                    }
                    else {
                        cell.innerHTML = rec_val;
                    }
                }
            }
            row.appendChild(cell);
        }
        tbody.appendChild(row);
    }
}

function applyChanges(table, changes) {
    const header_cells = table.querySelectorAll("thead th");
    const tbody = table.tBodies[0]

    let field_index = (fld_name) => {
        for (let i = 0; i < header_cells.length; i++) {
            if (header_cells[i].dataset.fieldName === fld_name) {
                return i;
            }
        }
        return undefined;
    }
    const run_id_ix = field_index("run_id");

    let find_row = (run_id) => {
        const rows = tbody.querySelectorAll("table tr");
        for (const row of rows) {
            const row_run_id = Number(row.children[run_id_ix].innerHTML);
            if (row_run_id === run_id) {
                return row;
            }
        }
        return undefined;
    }

    let add_change = (cell, fld_name, rec) => {
        let html = cell.innerHTML;
        let first_div = cell.firstElementChild;
        if (first_div && first_div.className === 'overridden-by-change') {
        }
        else {
            html = `<div class="overridden-by-change">${html}</div>`;
        }
        html += `<div style="font-weight: bold; color: darkred;">${rec.data.RunUpdateRequest[fld_name]}<a href="#" onclick="deleteChange(${rec.id})">❌</a></div>`
        html += `<div class="w3-small" style="color: darkblue;">${rec.user_id}</div>`
        if (rec.note) {
            html += `<div class="w3-small">${rec.note}</div>`
        }
        cell.innerHTML = html;
    }

    for (const rec of changes) {
        const run_id = rec.data_id;
        let row = find_row(run_id);
        if (row) {
            for (let fld_name in rec.data.RunUpdateRequest) {
                if (fld_name === 'run_id') {
                    continue;
                }
                const ix = field_index(fld_name);
                if (ix !== undefined) {
                    let cell = row.children[ix];
                    add_change(cell, fld_name, rec);
                }
            }
        }
    }
}
