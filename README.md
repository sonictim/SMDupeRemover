# SMDupeRemover
 CLI tool to find and remove duplicate filenames in Soundminer SQLITE databases.\
 This program only looks at filenames, not content.

##### USE AT YOUR OWN RISK I OFFER NO SUPPORT OF ANY KIND. BACK UP YOUR DATABASES BEFORE YOU USE THIS PROGRAM!
If you are unfailiar with how to run CLI tools or the terminal scares you, maybe this isn't for you.

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
    `SMDupeRemover <database> arguments`
    
- **CLI STUFF:**\
To run a program in a local directory you need to add './' So...  `./SMDupeRemover`\
You may also need to make sure that it has executable permissions:  `chmod +x SMDupeRemover`\
Again, if these are new concepts to you, you may not want to use this program.

## **FEATURES:**\
- Search Database for Duplicate Filenames and let it decide which ones to remove
- Customizable User Defined Logic for deciding which filenames to remove
- Database Comparison between two databases for overlapping filenames
- Search/Remove Files with Audio Suite Processing Tags
- Option to create database of just the records removed
- SAFETY: all work is done in a copy of the designated database

> NOTE: This program only deals with the database files.  After running the program, you can then mirror your library to reflect the changes, or use the duplicates database in soundminer to delete files.

#### I strongly suggest exploring the -s and -l flags when running this program.
These tags won't find as many duplicates to remove, but it's a much less overwhelming place to start when you want to figure out what it's removing.


## ARGUMENTS:

#### `--generate-config-files`
generates SMDupe_tags.txt and SMDupe_order.txt. SMDupe_order.txt defines how the main duplicate checker decides which file to keep.  this will overwrite what's there with the default values if they already exist in the directory.  I suggest running once and then modifying from there if you like.  Without these files, the program will just do the default order/tags I have pre programmed in the program.

#### `-c or --compare <database2>`
if any file in the target database exists in the comparison database, it will be removed

#### `-t or --prune-tags`
looks for common Protools Processing Tags and removes files with them.  can use SMDupe_tags.txt to define them.

#### `-n or --no-filename-check`
this doesn't run the normal duplicate filename check on the database.  Useful if you want to just remove tags or compare with another database only.

#### `-g or --group <column>`
Groups records by the specified column and then searches for duplicates within each group.  If the column data is NULL, those files will be skipped.
you can also specify `--group-null` and all NULL column data will be put into it's own group and searched for duplicates within this group.
This will override the -s and -l flags.

#### `-s or --group-by-show and -l or --group-by-library`
same as above but specifies either the show or library column.  Null entries are ignored.  use '--group-null show' to override. 

#### `-d or --create-duplicates-database`
after processing the database it will generate a new database containing all the deletions that were made

#### `-v or --verbose`
displays each file as it's being deleted.  Can FLOOD your terminal

#### `-y or --no-prompt`
automatically responds yes to the processing prompts

#### `-u or --unsafe`
writes DIRECTLY to the database.  Also, skips all the yes, no warnings.  USE WITH CAUTION.

#### `-h or --help`
gives a nice help summary

## CONFIGURATION:
SMDupeRemover has a built in logic and defaults but they can be overridden with the following configuration files.  
Use the --generate-config-files option to create/overwrite them with the default settings.

### `SMDupe_order.txt`

The default logic when comparing similar filenames on what to keep is:  

    duration DESC  
    channels DESC  
    sampleRate DESC  
    bitDepth DESC  
    BWDate ASC  
    scannedDate ASC  

DESC is descending, ASC is ascending. The higher up in the list, the higher the priority, so first it checks duration and works it's way down.  
You can really use any column you like from the Soundminer database and create your own custom order/logic.  I strongly recommend this.

For example, many duplicates in my library are a result of backing up protools sessions.  As a result, many of my dupes have "Audio Files" in their path.
So I add:  
##### `CASE WHEN pathname LIKE '%Audio Files%' THEN 1 ELSE 0 END ASC`
This will prioritize/save files that do not have "Audio Files" in their path over duplicates that have "Audio Files" in their path.
Two examples of this are generated in the comments for you when you create this config file via the `--generate-config-files` tag

### `SMDupe_tags.txt`
When processing audio files in protools, you can get lots of little tags added on to the end of filenames when creating this new media, but ultimately, it's a duplicate of something you already have in your library.  You can use SMDupe_tags.txt to designate what to look for and have removed from your library. You can designate any combination of characters you like.  I've put in a bunch I've found in my library as a default.  I suggest adding away.  I can also add more to the default you think I've missed.  Just send me a message.

SMDupe_tags.txt will only be processed with the --prune-tags (-t) option



 
    


