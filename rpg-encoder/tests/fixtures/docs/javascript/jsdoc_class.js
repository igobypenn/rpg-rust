/**
 * User management class.
 * 
 * This class provides methods for managing user data.
 * 
 * @class UserManager
 * @example
 * const manager = new UserManager();
 * manager.addUser({ name: 'John' });
 */
class UserManager {
    /**
     * Create a new UserManager instance.
     * 
     * @constructor
     * @param {Object} options - Configuration options
     * @param {string} options.storageType - Type of storage to use
     */
    constructor(options) {
        this.users = [];
    }
    
    /**
     * Add a new user to the manager.
     * 
     * @param {Object} user - User object to add
     * @param {string} user.name - User's name
     * @param {string} user.email - User's email
     * @returns {number} The new user count
     * @throws {Error} If user is invalid
     */
    addUser(user) {
        if (!user.name) {
            throw new Error('User must have a name');
        }
        this.users.push(user);
        return this.users.length;
    }
}
