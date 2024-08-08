use rusqlite::{Connection, Result};
use std::collections::HashSet;
use std::env;
use std::fs::{self, File};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::error::Error;
use terminal_size::{Width, terminal_size};

const DEFAULT_ORDER: [&str; 12] = [
    "CASE WHEN pathname LIKE '%Audio Files%' THEN 1 ELSE 0 END ASC",
    "CASE WHEN pathname LIKE '%RECORD%' THEN 0 ELSE 1 END ASC",
    "CASE WHEN pathname LIKE '%CREATED SFX%' THEN 0 ELSE 1 END ASC",
    "CASE WHEN pathname LIKE '%CREATED FX%' THEN 0 ELSE 1 END ASC",
    "CASE WHEN pathname LIKE '%LIBRARY%' THEN 0 ELSE 1 END ASC",
    "CASE WHEN pathname LIKE '%PULLS%' THEN 0 ELSE 1 END ASC",
    "duration DESC",
    "channels DESC",
    "sampleRate DESC",
    "bitDepth DESC",
    "BWDate ASC",
    "scannedDate ASC",
];

const DEFAULT_TAGS: [&str; 29] = [
    "-Reverse_", "-RVRS_", "-A2sA_", "-Delays_", "-ZXN5_", "-NYCT_", "-PiSh_", "-PnT2_", "-7eqa_",
    "-Alt7S_", "-AVrP_", "-X2mA_", "-PnTPro_", "-M2DN_", "-PSh_", "-ASMA_", "-TmShft_", "-Dn_",
    "-DVerb_", "-spce_", "-RX7Cnct_", "-AVSt", "-VariFi", "-DEC4_", "-VSPD_", "-6030_", "-NORM_",
    "-AVrT_", "-RING_"
];

fn fetch_filenames(conn: &Connection) -> Result<HashSet<String>> {
    let mut stmt = conn.prepare("SELECT filename FROM justinmetadata")?;
    let filenames: HashSet<String> = stmt.query_map([], |row| row.get(0))?
        .filter_map(Result::ok)
        .collect();
    Ok(filenames)
}

fn delete_filenames(conn: &mut Connection, filenames: &HashSet<String>) -> Result<()> {
    let tx = conn.transaction()?;
    
    // Convert the filenames into a vector
    let filename_vec: Vec<String> = filenames.iter().cloned().collect();
    
    // Split the filenames into batches if necessary
    const BATCH_SIZE: usize = 1000;
    for chunk in filename_vec.chunks(BATCH_SIZE) {
        let placeholders: Vec<String> = chunk.iter().map(|_| "?".to_string()).collect();
        let query = format!(
            "DELETE FROM justinmetadata WHERE filename IN ({})",
            placeholders.join(", ")
        );

        // Convert the chunk to a Vec<&dyn rusqlite::types::ToSql>
        let params: Vec<&dyn rusqlite::types::ToSql> = chunk.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();
        
        // Pass the parameters to `tx.execute`
        tx.execute(&query, params.as_slice())?;
    }
    
    tx.commit()?;

    // Run the VACUUM command
    // conn.execute("VACUUM", [])?;

    Ok(())
}


fn compare_duplicates(compare_db: &str, target_db: &str, unsafe_mode: bool) -> Result<usize> {
    println!("Comparing filenames between {} and {}", target_db, compare_db);
    
    let conn_a = Connection::open(compare_db)?;
    let mut conn_b = Connection::open(target_db)?;
    
    let filenames_a = fetch_filenames(&conn_a)?;
    let filenames_b = fetch_filenames(&conn_b)?;
    
    let common_filenames: HashSet<_> = filenames_a.intersection(&filenames_b).cloned().collect();
   
    let mut total = common_filenames.len();
    if total == 0 {
        println!("NO OVERLAPPING FILENAMES FOUND!");
        return Ok(0); // Exit the function early if no duplicates are found
    }
    
    if unsafe_mode {
        println!("Found {} overlapping filenames in {}. Proceeding with deletion.", total, target_db);
        delete_filenames(&mut conn_b, &common_filenames)?;
        println!("Removed {} files from {}", common_filenames.len(), target_db);
    } else {
        println!("Found {} overlapping filenames in {}. Type 'yes' to remove them: ", total, target_db);
        let mut user_input = String::new();
        io::stdin().read_line(&mut user_input).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
        let user_input = user_input.trim().to_lowercase();

        if user_input == "yes" {
            delete_filenames(&mut conn_b, &common_filenames)?;
            println!("Removed {} files from {}", common_filenames.len(), target_db);
        } else {
            total = 0;
            println!("Aborted deletion.");
        }
    }

    Ok(total)
}

// Function to read the order from the order file
fn read_order(file_path: &str) -> Result<Vec<String>, io::Error> {
    let path = Path::new(file_path);

    if path.exists() {
        let file = File::open(path)?;
        let reader = io::BufReader::new(file);

        let lines: Vec<String> = reader.lines()
            .filter_map(|line| line.ok())
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect();

        Ok(lines)
    } else {
        // If the file doesn't exist, return DEFAULT_ORDER
        Ok(DEFAULT_ORDER.iter().map(|&s| s.to_string()).collect())
    }
}

fn remove_duplicates(db_path: &str, unsafe_mode: bool, verbose: bool, group: &str, skip_null: bool) -> Result<usize, rusqlite::Error> {
    println!("Searching for Duplicate Filenames in: {}", db_path);

    let mut conn = Connection::open(db_path)?;

    // Read the order file
    let order_file = "SMDupe_order.txt";
    let order = read_order(order_file).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;

    // Construct the ORDER BY clause dynamically
    let order_clause = order.join(", ");

    // Start a transaction
    let tx = conn.transaction()?;

    let mut group1 = "".to_string();
    let mut group2 = "".to_string();
    if group != "" {group1 = format!("{}, ", group);}
    if group != "" && skip_null {group2 = format!("WHERE {} IS NOT NULL AND {} != ''", group, group);}
    
    // Find the best record for each filename based on the given criteria
    let mut ids_to_delete: Vec<(i64, String)> = {
        let sql = format!(
            "
            WITH ranked AS (
                SELECT
                    rowid AS id,
                    filename,
                    ROW_NUMBER() OVER (
                        PARTITION BY {} filename
                        ORDER BY {}
                    ) as rn
                FROM justinmetadata {}
            )
            SELECT id, filename FROM ranked WHERE rn > 1
            ",
            group1, order_clause, group2
        );

        let mut stmt = tx.prepare(&sql)?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.filter_map(Result::ok).collect()
    }; // stmt is dropped here
    ids_to_delete.sort_by(|a, b| b.0.cmp(&a.0));
    let mut total = ids_to_delete.len();

    if total == 0 {
        println!("ALL FILENAMES are UNIQUE! in {}", db_path);
        return Ok(0); // Exit the function early if no duplicates are found
    }

    //Get Terminal Size Width for the verbose mode
    let (width, _) = terminal_size().unwrap_or((Width(80), terminal_size::Height(0)));
    let line_width = width.0 as usize;

    if unsafe_mode {
        if group != "" {println!("Grouping by {}", group)};
        println!("Found {} Duplicate Filenames. Proceeding with deletion.", total);
        // Delete the records that are not the best
        for (id, filename) in ids_to_delete {
            if verbose {print!("\r{}", " ".repeat(line_width)); print!("\rRemoving ID: {}, Filename: {}", id, filename); let _ = io::stdout().flush();}
            tx.execute("DELETE FROM justinmetadata WHERE rowid = ?", [id])?;
        }
        tx.commit()?;
        // conn.execute("VACUUM", [])?;
        println!("\nRemoved {} files from {}", total, db_path);
    } else {
        if group != "" {println!("Grouping by {}", group)};
        println!("Found {} Duplicate Filenames. Type 'yes' to remove them: ", total);
        let mut user_input = String::new();
        io::stdin().read_line(&mut user_input).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
        let user_input = user_input.trim().to_lowercase();

        if user_input == "yes" {
            // Delete the records that are not the best
            for (id, filename) in ids_to_delete {
                if verbose {print!("\r{}", " ".repeat(line_width)); print!("\rRemoving ID: {}, Filename: {}", id, filename); let _ = io::stdout().flush();}
                tx.execute("DELETE FROM justinmetadata WHERE rowid = ?", [id])?;
            }
            tx.commit()?;
            // conn.execute("VACUUM", [])?;
            println!("\nRemoved {} Entries from {}", total, db_path);
        } else {
            total = 0;
            println!("Aborted deletion.");
        }
    }

    Ok(total)
}

fn read_tags(file_path: &str) -> Result<Vec<String>, rusqlite::Error> {
    let path = Path::new(file_path);
    
    if path.exists() {
        let file = File::open(&path).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
        let reader = io::BufReader::new(file);
        let tags: Vec<String> = reader.lines()
            .filter_map(|line| {
                let line = line.ok()?;
                let trimmed_line = line.trim().to_string();
                if trimmed_line.is_empty() {
                    None
                } else {
                    Some(trimmed_line)
                }
            })
            .collect();
        Ok(tags)
    } else {
        // Use DEFAULT_TAGS if the file doesn't exist
        let default_tags: Vec<String> = DEFAULT_TAGS.iter().map(|&s| s.to_string()).collect();
        Ok(default_tags)
    }
}


fn prune_tags(db_path: &str, tags_filename: &str, unsafe_mode: bool, verbose: bool) -> Result<usize> {
    let mut conn = Connection::open(db_path)?;
    let tags = read_tags(tags_filename)?;

    // println!("TAGS!");
    // for tag in &tags {
    //     println!("{}", tag);
    // }

    // Count the number of rows that match the tags
    let mut total_rows_found = count_rows_with_tags(&mut conn, &tags, verbose)?;

    if total_rows_found == 0 {
        println!("No rows found with the specified tags.");
        return Ok(0);
    }

    if unsafe_mode {
        println!("Found {} filenames with matching tags. Proceeding with deletion.", total_rows_found);
        let rows_deleted = delete_rows_with_tags(&mut conn, &tags, verbose)?;
        println!("Deleted {} Entries from {}", rows_deleted, db_path);
    } else {
        // Prompt the user
        println!("Found {} filenames with matching tags. Type 'yes' to remove them from {}.", total_rows_found, db_path);
        let mut user_input = String::new();
        io::stdin().read_line(&mut user_input).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
        let user_input = user_input.trim().to_lowercase();

        if user_input == "yes" {
            // If user confirms, remove the rows
            let rows_deleted = delete_rows_with_tags(&mut conn, &tags, verbose)?;
            println!("Deleted {} rows from {}", rows_deleted, db_path);
        } else {
            total_rows_found = 0;
            println!("Aborted deletion.");
        }
    }

    Ok(total_rows_found)
}

fn delete_rows_with_tags(conn: &mut Connection, tags: &[String], verbose: bool) -> Result<usize> {
    let tx = conn.transaction()?;
    let mut total_rows_deleted = 0;

    for tag in tags {
        let query = format!("DELETE FROM justinmetadata WHERE filename LIKE '%' || ? || '%'");
        let rows_deleted = tx.execute(&query, &[tag])?;
        if rows_deleted > 0 && verbose {
            println!("Filenames removed for tag '{}': {}", tag, rows_deleted);
        }

        total_rows_deleted += rows_deleted;
    }

    tx.commit()?;
    // conn.execute("VACUUM", [])?;
    Ok(total_rows_deleted)
}

fn count_rows_with_tags(conn: &mut Connection, tags: &[String], verbose: bool) -> Result<usize> {
    let tx = conn.transaction()?;
    let mut total_rows_found = 0;
    let mut processed_files = std::collections::HashSet::new();

    for tag in tags {
        let query = format!("SELECT filename FROM justinmetadata WHERE filename LIKE '%' || ? || '%'");
        let mut stmt = tx.prepare(&query)?;
        let rows = stmt.query_map(&[tag], |row| row.get::<_, String>(0))?;

        let mut count = 0;
        for filename in rows {
            let filename = filename?;
            if processed_files.insert(filename.clone()) {
                count += 1;
            }
        }
        
        if count > 0 && verbose {
            println!("Filenames found for tag '{}': {}", tag, count);
        }
        total_rows_found += count;
    }

    tx.commit()?;
    Ok(total_rows_found)
}

//REMOVE ROWS THAT ARE IN BOTH DBs.... will filter dups database and leave only the dupes behind
fn remove_matching_rows(dupe_db_path: &str, processed_db_path: &str) -> Result<()> {
    let mut dupe_conn = Connection::open(dupe_db_path)?;
    let processed_conn = Connection::open(processed_db_path)?;

    
    // Get IDs of rows in the processed database
    let ids_to_remove: Vec<i64> = {
        let mut stmt = processed_conn.prepare("SELECT rowid FROM justinmetadata")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.filter_map(Result::ok).collect()
    };

    delete_rows_in_batches(&mut dupe_conn, ids_to_remove)?;

    dupe_conn.execute("VACUUM", [])?; // Execute VACUUM on the dupe connection   
 
    // Get the count of remaining rows
    let remaining_count: usize = dupe_conn.query_row(
        "SELECT COUNT(*) FROM justinmetadata",
        [],
        |row| row.get(0)
    )?;

    println!("\n{} records moved to {}", remaining_count, dupe_db_path);

    Ok(())
}

fn delete_rows_in_batches(conn: &mut Connection, ids_to_remove: Vec<i64>) -> Result<()> {
    let tx = conn.transaction()?;

    // Split the ids into batches if necessary
    const BATCH_SIZE: usize = 10000;
    for chunk in ids_to_remove.chunks(BATCH_SIZE) {
        let _ = io::stdout().flush();
        print!(".");
        let placeholders: Vec<String> = chunk.iter().map(|_| "?".to_string()).collect();
        let query = format!(
            "DELETE FROM justinmetadata WHERE rowid IN ({})",
            placeholders.join(", ")
        );

        // Convert the chunk to a Vec<&dyn rusqlite::types::ToSql>
        let params: Vec<&dyn rusqlite::types::ToSql> = chunk.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();

        // Pass the parameters to `tx.execute`
        tx.execute(&query, params.as_slice())?;
    }

    tx.commit()?;

    Ok(())
}


fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    
    let mut generate_config_files = false;
    let mut primary_db: Option<String> = None;
    let mut prune_tags_flag = false;
    let mut no_filename_check = false;
    let mut compare_db: Option<String> = None;
    let mut unsafe_mode = false;
    let mut just_say_yes = false;
    let mut duplicates_database = false;
    let mut duplicate_db_path = "".to_string();
    let mut verbose = false;
    let mut group = "";
    let mut skip_null = true;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--generate-config-files" => generate_config_files = true,
            "--prune-tags" | "-t" => prune_tags_flag = true,
            "--no-filename-check" | "-n" => no_filename_check = true,
            "--group-by-show" | "-s" => group = "show",
            "--group-by-library" | "-l" => group = "library",
            "--group" | "-g" => {
                if i + 1 < args.len() {
                    group = args[i + 1].as_str();
                    i += 1; // Skip the next argument since it's the database name
                } else {
                    print_help();
                    return Ok(());
                }
            },
            "--group-null" => {
                if i + 1 < args.len() {
                    group = args[i + 1].as_str();
                    i += 1; // Skip the next argument since it's the database name
                    skip_null = false;
                } else {
                    print_help();
                    return Ok(());
                }
            },
            "--compare" | "-c" => {
                if i + 1 < args.len() {
                    compare_db = Some(args[i + 1].clone());
                    i += 1; // Skip the next argument since it's the database name
                } else {
                    print_help();
                    return Ok(());
                }
            },
            "--no-prompt" | "--yes" | "-y" => just_say_yes = true,
            "--unsafe" | "-u" => {
                unsafe_mode = true;
                just_say_yes = true;
            },
            "--create-duplicates-database" | "-d" => duplicates_database = true,
            "--verbose" | "-v" => verbose = true,
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            _ => {
                if args[i].starts_with('-') && !args[i].starts_with("--") {
                    for c in args[i][1..].chars() {
                        match c {
                            'g' => generate_config_files = true,
                            't' => prune_tags_flag = true,
                            'n' => no_filename_check = true,
                            's' => group = "show",
                            'l' => group = "library",
                            'y' => just_say_yes = true,
                            'u' => {
                                unsafe_mode = true;
                                just_say_yes = true;
                            },
                            'd' => duplicates_database = true,
                            'v' => verbose = true,
                            'h' => {
                                print_help();
                                return Ok(());
                            },
                            'c' => {
                                if i + 1 < args.len() {
                                    compare_db = Some(args[i + 1].clone());
                                    i += 1; // Skip the next argument since it's the database name
                                } else {
                                    print_help();
                                    return Ok(());
                                }
                            },
                            _ => {
                                println!("Unknown option: -{}", c);
                                print_help();
                                return Ok(());
                            }
                        }
                    }
                } else {
                    if primary_db.is_none() {
                        primary_db = Some(args[i].clone());
                    } else {
                        print_help();
                        return Ok(());
                    }
                }
            }
        }
        i += 1;
    }

    if generate_config_files {
        // Generate SMDupe_order.txt and SMDupe_tags.txt with default values
        let order_file_path = "SMDupe_order.txt";

        let mut order_file = File::create(order_file_path)?;
        writeln!(order_file, "## Column in order of Priority and whether it should be DESCending or ASCending.  Hashtag will bypass")?;
        for field in &DEFAULT_ORDER {
            writeln!(order_file, "{}", field)?;
        }

        println!("Created {} with default order.", order_file_path);

        let tags_file_path = "SMDupe_tags.txt";

        let mut tags_file = File::create(tags_file_path)?;
        for tag in DEFAULT_TAGS {
            writeln!(tags_file, "{}", tag)?;
        }

        println!("Created {} with default tags.", tags_file_path);

        // Exit if no other arguments
        if primary_db.is_none() && compare_db.is_none() {
            return Ok(());
        }
    }

    if let Some(db_path) = primary_db {
        if !Path::new(&db_path).exists() {
            println!("Error: Primary database {} does not exist.", db_path);
            return Ok(());
        }
        if duplicates_database {
            duplicate_db_path = format!("{}_dupes.sqlite", db_path.trim_end_matches(".sqlite"));
            fs::copy(&db_path, &duplicate_db_path)?;    
        }

        let target_db_path = if unsafe_mode {
            db_path.clone()
        } else {
            let new_db_path = format!("{}_thinned.sqlite", db_path.trim_end_matches(".sqlite"));
            fs::copy(&db_path, &new_db_path)?;
            new_db_path
        };

        let mut total: usize = 0;
        if let Some(compare_db_path) = compare_db {
            if !Path::new(&compare_db_path).exists() {
                println!("Error: Compare database {} does not exist.", compare_db_path);
                return Ok(());
            }
            total += compare_duplicates(&compare_db_path, &target_db_path, just_say_yes)?;
        }

        if !no_filename_check {
            total += remove_duplicates(&target_db_path, just_say_yes, verbose, group, skip_null)?;
        }

        if prune_tags_flag {
            total += prune_tags(&target_db_path, "SMDupe_tags.txt", just_say_yes, verbose)?;
        }
        if total > 0 {
            
            println!("Cleaning up Database");
            let conn = Connection::open(&target_db_path)?;
            conn.execute("VACUUM", [])?; // Execute VACUUM on the database
            println!("{} Total Entries removed from Database", total);

        }


        if duplicates_database {
            println!("Generating Duplicates Only Database (this can be slow if your database is huge)");
            let _ = remove_matching_rows(&duplicate_db_path, &target_db_path)?;
            // let _ = compare_duplicates(&target_db_path, &duplicate_db_path, true)?;
        }
    } else {
        print_help();
    }

    Ok(())
}


fn print_help() {
    let help_message = "
Usage: SMDupeRemover <database> [options]

Options:
    -c, --compare <database>          Compare with another database
    -d, --create-duplicates-database  Generates an additional _dupes database of all files that were removed
        --generate-config-files       Generate default config files (SMDupe_order.txt and SMDupe_tags.txt)
    -g, --group <column>              Search for Duplicates within the specified column groupings.  NULL column records skipped
        --group-null <column>         Search for Duplicates within the specified column groupings.  NULL column records processed together
    -h, --help                        Display this help message
    -l, --group-by-library            Search for duplicates within each Library. Untagged Library files skipped
    -n, --no-filename-check           Skips searching for filename duplicates in main database
    -s, --group-by-show               Search for duplicates within each show. Untagged Show files skipped
    -t, --prune-tags                  Remove Files with Specified Tags in SMDupe_tags.txt or use defaults
    -u, --unsafe                      WRITES DIRECTLY TO TARGET DATABASE with NO PROMPT
    -v, --verbose                     Display Additional File Processing Details
    -y, --no-prompt                   Auto Answer YES to all prompts

Arguments:
    <database>                        Path to the primary database

Examples:
    smduperemover mydatabase.sqlite --prune-tags
    smduperemover mydatabase.sqlite -p -g
    smduperemover mydatabase.sqlite -pvu
    smduperemover mydatabase.sqlite --compare anotherdatabase.sqlite

Configuration:
    SMDupe_order.txt defines the order of data (colums) checked when deciding on the logic of which file to keep
    SMDupe_tags.txt is a list of character combinations that if found in the filename, it will be removed with the -p option

Description:
    SMDupeRemover is a tool for removing duplicate filename entries from a Soundminer database.
    It can generate configuration files, prune tags, and compare databases.
";

    println!("{}", help_message);
}


