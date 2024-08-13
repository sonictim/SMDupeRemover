fn gather_duplicate_filenames_in_database2(conn: &mut Connection) -> Result<HashSet<FileRecord>, rusqlite::Error> {
    print_filenames_and_filepaths(conn)?;
    println!("SQL found {} unique duplicate filenames", count_unique_duplicate_filenames(conn).unwrap());
    let records = fetch_filerecords_from_database(&conn)?;
    let mut filtered_map = convert_and_filter(records);
    println!("My function found {} unique filenames", filtered_map.len());
    Ok(remove_best_match(&conn, filtered_map).unwrap())
 
}

fn print_filenames_and_filepaths(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare(
        "
        SELECT filename, filepath
        FROM justinmetadata
        WHERE filename LIKE '%Flight INT%'
        ORDER BY filename, filepath
        "
    )?;

    let rows = stmt.query_map([], |row| {
        let filename: String = row.get(0)?;
        let filepath: String = row.get(1)?;
        Ok((filename, filepath))
    })?;

    let mut filename_map: HashMap<String, Vec<String>> = HashMap::new();

    for row_result in rows {
        match row_result {
            Ok((filename, filepath)) => {
                filename_map.entry(filename).or_default().push(filepath);
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    for (filename, filepaths) in filename_map {
        if filepaths.len() > 1 {
            println!("Filename: {}", filename);
            println!("Count: {}", filepaths.len());
            for filepath in filepaths {
                println!("  Filepath: {}", filepath);
            }
        }
    }

    Ok(())
}

fn print_filenames_with_counts(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare(
        "
        SELECT filename, COUNT(*) as count
        FROM justinmetadata
        GROUP BY filename
        HAVING COUNT(*) > 1
        "
    )?;

    let rows = stmt.query_map([], |row| {
        let filename: String = row.get(0)?;
        let count: i32 = row.get(1)?;
        Ok((filename, count))
    })?;

    for row in rows {
        match row {
            Ok((filename, count)) => println!("Filename: {}, Count: {}", filename, count),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    Ok(())
}

fn print_filenames_with_multiple_entries(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare(
        "
        SELECT filename
        FROM justinmetadata
        GROUP BY filename
        HAVING COUNT(*) > 1
        "
    )?;

    let rows = stmt.query_map([], |row| {
        let filename: String = row.get(0)?;
        Ok(filename)
    })?;

    for row in rows {
        match row {
            Ok(filename) => println!("{}", filename),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    Ok(())
}

fn convert_and_filter(records: HashSet<FileRecord>) -> HashMap<String, HashSet<usize>> {
    // Step 1: Create the HashMap
    let mut map: HashMap<String, HashSet<usize>> = HashMap::new();
    
    for record in records {
        map.entry(record.filename)
        .or_insert_with(HashSet::new)
        .insert(record.id);
}

// Step 2: Filter out entries where the HashSet of record IDs has fewer than 2 elements
map.retain(|_, ids| ids.len() > 1);

map
}

fn convert_to_filerecords(map: HashMap<String, HashSet<usize>>) -> HashSet<FileRecord> {
    let mut filerecords = HashSet::new();
    
    for (filename, ids) in map {
        for id in ids {
            filerecords.insert(FileRecord {
                filename: filename.clone(),
                id: id,
            });
        }
    }
    
    filerecords
}

fn get_best_record_id(conn: &Connection, filename: &str) -> Result<usize> {
    let mut stmt = conn.prepare(
        "
        SELECT record_id
        FROM justinmetadata
        WHERE filename = ?
        ORDER BY 
        duration DESC,       -- 1. Prefer longer duration
        samplerate ASC,      -- 2. If duration is the same, prefer lower sample rate
        entrydate DESC       -- 3. If sample rate is the same, prefer more recent entry date
        LIMIT 1                 -- Get the best match
        "
    )?;
    
    let record_id: usize = stmt.query_row([filename], |row| row.get(0))?;
    Ok(record_id)
}

fn remove_best_match(conn: &Connection, mut map: HashMap<String, HashSet<usize>>) -> Result<HashSet<FileRecord>> {
    for (filename, ids) in &mut map {
        if ids.len() > 1 {
            if let Ok(best_record_id) = get_best_record_id(conn, filename) {
                // Remove the best match from the HashSet
                ids.remove(&best_record_id);
            }
        }
    }
    let result = convert_to_filerecords(map);
    println!("{} Records marked for deletion", result.len());
    Ok(result)
}

