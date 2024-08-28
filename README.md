# SMDupeRemover
 CLI tool to find and remove duplicate filenames in Soundminer SQLITE databases.\
 This program only looks at filenames, not content.

> **USE AT YOUR OWN RISK I OFFER NO SUPPORT OF ANY KIND. BACK UP YOUR DATABASES BEFORE YOU USE THIS PROGRAM!  
If you are unfailiar with how to run CLI tools or the terminal scares you, maybe this isn't for you.**

## **FEATURES:**
- Search Database for Duplicate Filenames and let it decide which ones to remove
- Customizable User Defined Logic for deciding which filenames to remove
- Database Comparison between two databases for overlapping filenames
- Search/Remove Files with Audio Suite Processing Tags
- Option to create database of just the records removed
- SAFETY: all work is done in a copy of the designated database

> NOTE: This program only deals with the database files.  After running the program, you can then mirror your library to reflect the changes,
> or create a duplicates database and open with soundminer and use it to help you decide what to delete

I strongly suggest exploring the -s and -l flags when first running this program.  These tags won't find as many duplicates to remove, but it's a much less overwhelming place to start when you want to figure out how the program works and what it's removing.

## INSTALLATION:
- **DOWNLOAD:**  
If you are a mac user, the compiled binary is found in the `Mac Universal Binary` folder.
I find it easiest to copy it to the same folder as your Soundminer Databases and run from there.\
If you don't know where those are, you probably shouldn't be running this program.

- **BUILD:**\
It's written in rust.  If you know how to build things in rust, then go nuts!
I also made a little build script that helps me make the mac universal binary and copies the final program to my Soundminer Databases Folder.\
If you know how to build, then you should know how to update this to your needs

## USAGE: 
    `SMDupeRemover <target_database> [arguments]`
    
- **CLI STUFF:**\
To run a program in a local directory you need to add './' So...  `./SMDupeRemover`\
You may also need to make sure that it has executable permissions:  `chmod +x SMDupeRemover`\
Again, if these are new concepts to you, you may not want to use this program.

## ARGUMENTS:

#### `--generate-config-files`
Generates `SMDupe_tags.txt` and `SMDupe_order.txt` SMDupe_order.txt defines how the main duplicate checker decides which file to keep.  this will overwrite what's there with the default values if they already exist in the directory.  I suggest running once and then modifying from there if you like.  Without these files, the program will just do the default order/tags I have pre-programmed in the program.

#### `-c or --compare <comparison_database>`
If any file in the target database exists in the comparison database, it will be marked for deletion in the target database

#### `-D or --deep-dive`
Looks for duplicates among filenames with extra .1 or .M at the end of the filename.  
For example crash.flac, crash.1.flac, crash.1.2.1.flac, and crash.M.flac will be grouped together and then sorted by duraion.  Duplicates are then searched for amongst these sorted groups

#### `-t or --prune-tags`
Looks for common Protools Processing Tags and removes files with them.  Can use `SMDupe_tags.txt` to define them.

#### `-a or --all`
Searches for duplicates, checks tags, deep dive search, and creates a duplicates only database after deletion.

#### `-n or --no-filename-check`
Skips the normal duplicate filename check on the database.  Useful if you want to just remove tags or compare with another database only.

#### `-g or --group <column>`
Groups records by the specified column and then searches for duplicates within each group.  If the column data is NULL, those files will be skipped.
you can also specify `--group-null` and all NULL column data will be put into it's own group and searched for duplicates within this group.
This will override the -s and -l flags.

#### `-s or --group-by-show and -l or --group-by-library`
Same as above but specifies either the show or library column.  Null entries are ignored.  use '--group-null show' to override. 

#### `-d or --create-duplicates-database`
After processing the target database it will generate a new database containing all the deleted records

#### `-v or --verbose`
Displays each file as it's being deleted and some additional processing information.

#### `-y or --no-prompt`
Automatically responds yes to the processing prompts

#### `-u or --unsafe`
Skips the safety prompt and overwrites your database after deletion 

#### `-h or --help`
Reminds you how to use the program

## CONFIGURATION:
SMDupeRemover has a built in logic and defaults but they can be overridden with the following configuration files.  
Use the --generate-config-files option to create/overwrite them with the default settings.

### SMDupe_tags.txt
When processing audio files in protools via Audio Suite, you can get lots of little tags added on to the end of filenames when creating this new media, but ultimately, it's a duplicate of something you already have in your library.  `SMDupe_tags.txt` is meant to be a list of these tags, but you can put **any text** you want to use as a flag for deletion in this list.

> NOTE: `SMDupe_tags.txt` will only be processed with the --prune-tags or -t option

The -v option will also display what tags it is searching for and how many it finds for each tag

### SMDupe_order.txt
This file allows you to create your own Logic for how the program decides which file to keep when it finds duplicates.  It uses SQL ORDER logic.

The default logic when comparing similar filenames on what to keep is: 

> duration DESC  
    channels DESC  
    sampleRate DESC  
    bitDepth DESC  
    BWDate ASC  
    scannedDate ASC

DESC is descending, ASC is ascending. The higher up in the list, the higher the priority, so first it checks duration and works it's way down.

You can really **use any column** you like from the Soundminer database and create your own custom order/logic.  In my own Library, I've had the **MOST SUCCESS**
creating custom decisions in regards to the filepath.  

For example, my library is split into two main forks, *LIBRARIES* and *SHOWS*.  My *SHOWS* fork has not only show library subfolders, but also lots of backups of old sessions. These session backups tend to have files in an *Audio Files* folder.  Keeping both of these facts in mind, here's how I steer the logic.
So I add:  

> CASE WHEN pathname LIKE '%LIBRARIES%' THEN 0 ELSE 1 END ASC  
CASE WHEN pathname LIKE '%Audio Files%' THEN 1 ELSE 0 END ASC

The first line is will prioritize any file in my *LIBRARIES* fork over anything in the *SHOWS* fork.  
The second line prioritize deleting records with *Audio Files* in their path over files that do not contain it.

Two examples of this are generated in the comments for you when you create this config file via the `--generate-config-files` tag.  

It's not too hard to figure out, but I found ChatGPT to be very helpful.  Just ask it "In SQLite, I'm trying to order by filepath where files without 'Audio Files' in their path get chosen first"  and it will help you come up with the statement you need.

If you are curious, this is my full config that currently works best for how I have my library organized.  **YMMV**

> CASE WHEN pathname LIKE '%TJF RECORDINGS%' THEN 0 ELSE 1 END ASC\
CASE WHEN pathname LIKE '%LIBRARIES%' THEN 0 ELSE 1 END ASC\
CASE WHEN pathname LIKE '%SHOWS/Tim Farrell%' THEN 1 ELSE 0 END ASC\
CASE WHEN Description IS NOT NULL AND Description != '' THEN 0 ELSE 1 END ASC\
CASE WHEN pathname LIKE '%Audio Files%' THEN 1 ELSE 0 END ASC\
CASE WHEN pathname LIKE '%RECORD%' THEN 0 ELSE 1 END ASC\
CASE WHEN pathname LIKE '%CREATED SFX%' THEN 0 ELSE 1 END ASC\
CASE WHEN pathname LIKE '%CREATED FX%' THEN 0 ELSE 1 END ASC\
CASE WHEN pathname LIKE '%LIBRARY%' THEN 0 ELSE 1 END ASC\
CASE WHEN pathname LIKE '%/LIBRARY%' THEN 0 ELSE 1 END ASC\
CASE WHEN pathname LIKE '%LIBRARY/%' THEN 0 ELSE 1 END ASC\
CASE WHEN pathname LIKE '%SIGNATURE%' THEN 0 ELSE 1 END ASC\
CASE WHEN pathname LIKE '%PULLS%' THEN 0 ELSE 1 END ASC\
CASE WHEN pathname LIKE '%EDIT%' THEN 1 ELSE 0 END ASC\
CASE WHEN pathname LIKE '%MIX%' THEN 1 ELSE 0 END ASC\
CASE WHEN pathname LIKE '%SESSION%' THEN 1 ELSE 0 END ASC\
duration DESC\
channels DESC\
sampleRate DESC\
bitDepth DESC\
BWDate ASC\
scannedDate ASC






 
    


